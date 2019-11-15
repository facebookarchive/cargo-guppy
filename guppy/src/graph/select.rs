// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::{
    DependencyDirection, DependencyEdge, DependencyLink, PackageGraph, PackageMetadata,
};
use crate::petgraph_support::reversed::ReversedDirected;
use crate::petgraph_support::walk::EdgeDfs;
use crate::{Error, PackageId};
use derivative::Derivative;
use fixedbitset::FixedBitSet;
use petgraph::prelude::*;
use petgraph::visit::{IntoNeighbors, NodeFiltered, Topo, VisitMap, Visitable};

/// A selector over a package graph.
///
/// This is the entry point for iterators over IDs and dependency links, and dot graph presentation.
/// A `PackageSelect` is constructed through the `select_` methods on `PackageGraph`.
#[derive(Clone, Debug)]
pub struct PackageSelect<'g> {
    // The fields are pub(super) for access within the graph module.
    pub(super) package_graph: &'g PackageGraph,
    pub(super) params: PackageSelectParams,
}

/// ## Selectors
///
/// The methods in this section create *package selectors*, which are queries over subsets of this
/// package graph. Use the methods here for queries based on transitive dependencies.
impl PackageGraph {
    /// Creates a new selector that returns all members of this package graph.
    pub fn select_all<'g>(&'g self) -> PackageSelect<'g> {
        PackageSelect {
            package_graph: self,
            params: PackageSelectParams::All,
        }
    }

    /// Creates a new selector that returns transitive dependencies of the given packages in the
    /// specified direction.
    ///
    /// Returns an error if any package IDs are unknown.
    pub fn select_directed<'g, 'a>(
        &'g self,
        package_ids: impl IntoIterator<Item = &'a PackageId>,
        dep_direction: DependencyDirection,
    ) -> Result<PackageSelect<'g>, Error> {
        match dep_direction {
            DependencyDirection::Forward => self.select_forward(package_ids),
            DependencyDirection::Reverse => self.select_reverse(package_ids),
        }
    }

    /// Creates a new selector that returns transitive dependencies of the given packages in the
    /// specified direction.
    ///
    /// Returns an error if any package IDs are unknown.
    #[deprecated(since = "0.1.3", note = "renamed to `select_directed`")]
    pub fn select_transitive_deps_directed<'g, 'a>(
        &'g self,
        package_ids: impl IntoIterator<Item = &'a PackageId>,
        dep_direction: DependencyDirection,
    ) -> Result<PackageSelect<'g>, Error> {
        self.select_directed(package_ids, dep_direction)
    }

    /// Creates a new selector that returns transitive dependencies of the given packages.
    ///
    /// Returns an error if any package IDs are unknown.
    pub fn select_forward<'g, 'a>(
        &'g self,
        package_ids: impl IntoIterator<Item = &'a PackageId>,
    ) -> Result<PackageSelect<'g>, Error> {
        Ok(PackageSelect {
            package_graph: self,
            params: PackageSelectParams::SelectForward(self.node_idxs(package_ids)?),
        })
    }

    /// Creates a new selector that returns transitive dependencies of the given packages.
    ///
    /// Returns an error if any package IDs are unknown.
    #[deprecated(since = "0.1.3", note = "renamed to `select_forward`")]
    pub fn select_transitive_deps<'g, 'a>(
        &'g self,
        package_ids: impl IntoIterator<Item = &'a PackageId>,
    ) -> Result<PackageSelect<'g>, Error> {
        self.select_forward(package_ids)
    }

    /// Creates a new selector that returns transitive reverse dependencies of the given packages.
    ///
    /// Returns an error if any package IDs are unknown.
    pub fn select_reverse<'g, 'a>(
        &'g self,
        package_ids: impl IntoIterator<Item = &'a PackageId>,
    ) -> Result<PackageSelect<'g>, Error> {
        Ok(PackageSelect {
            package_graph: self,
            params: PackageSelectParams::SelectReverse(self.node_idxs(package_ids)?),
        })
    }

    /// Creates a new selector that returns reverse transitive dependencies of the given packages.
    ///
    /// Returns an error if any package IDs are unknown.
    #[deprecated(since = "0.1.3", note = "renamed to `select_reverse`")]
    pub fn select_transitive_reverse_deps<'g, 'a>(
        &'g self,
        package_ids: impl IntoIterator<Item = &'a PackageId>,
    ) -> Result<PackageSelect<'g>, Error> {
        self.select_forward(package_ids)
    }
}

impl<'g> PackageSelect<'g> {
    /// Returns the set of "root packages" in the specified direction.
    ///
    /// * If direction is Forward, return the set of packages that do not have any dependencies
    ///   within the selected graph.
    /// * If direction is Reverse, return the set of packages that do not have any dependents within
    ///   the selected graph.
    pub fn into_root_ids(
        self,
        direction: DependencyDirection,
    ) -> impl Iterator<Item = &'g PackageId> + ExactSizeIterator + 'g {
        let dep_graph = self.package_graph.dep_graph();
        let (_, roots) = select_postfilter(dep_graph, self.params, direction);
        roots.into_iter().map(move |node_idx| &dep_graph[node_idx])
    }

    /// Consumes this query and creates an iterator over package IDs, returned in topological order.
    ///
    /// The default order of iteration is determined by the type of query:
    /// * for `all` and `forward` queries, package IDs are returned in forward order.
    /// * for `reverse` queries, package IDs are returned in reverse order.
    pub fn into_iter_ids(self, direction_opt: Option<DependencyDirection>) -> IntoIterIds<'g> {
        let direction = direction_opt.unwrap_or_else(|| self.params.default_direction());
        let dep_graph = self.package_graph.dep_graph();

        // If the topo order guarantee weren't present, this could potentially be done in a lazier
        // fashion, where reachable nodes are discovered dynamically. However, there's value in
        // topological ordering so pay the cost of computing the reachable map upfront. As a bonus,
        // this approach also allows the iterator to implement ExactSizeIterator.
        let (reachable, count) = select_prefilter(dep_graph, self.params);
        let filtered_graph = NodeFiltered(dep_graph, reachable);

        let topo = match direction {
            DependencyDirection::Forward => Topo::new(&filtered_graph),
            DependencyDirection::Reverse => Topo::new(ReversedDirected(&filtered_graph)),
        };
        IntoIterIds {
            graph: filtered_graph,
            topo,
            direction,
            remaining: count,
        }
    }

    /// Returns the set of "root package" metadata in the specified direction.
    ///
    /// * If direction is Forward, return the set of metadatas that do not have any dependencies
    ///   within the selected graph.
    /// * If direction is Reverse, return the set of metadatas that do not have any dependents within
    ///   the selected graph.
    pub fn into_root_metadatas(
        self,
        direction: DependencyDirection,
    ) -> impl Iterator<Item = &'g PackageMetadata> + ExactSizeIterator + 'g {
        let dep_graph = self.package_graph.dep_graph();
        let (_, roots) = select_postfilter(dep_graph, self.params.clone(), direction);
        roots.into_iter().map(move |node_idx| {
            self.package_graph
                .metadata(&dep_graph[node_idx])
                .expect("invalid node index")
        })
    }

    /// Consumes this query and creates an iterator over `PackageMetadata` instances, returned in
    /// topological order.
    ///
    /// The default order of iteration is determined by the type of query:
    /// * for `all` and `forward` queries, package IDs are returned in forward order.
    /// * for `reverse` queries, package IDs are returned in reverse order.
    pub fn into_iter_metadatas(
        self,
        direction_opt: Option<DependencyDirection>,
    ) -> IntoIterMetadatas<'g> {
        let package_graph = self.package_graph;
        let inner = self.into_iter_ids(direction_opt);
        IntoIterMetadatas {
            package_graph,
            inner,
        }
    }

    /// Consumes this query and creates an iterator over dependency links.
    ///
    /// If the iteration is in forward order, for any given package, at least one link where the
    /// package is on the `to` end is returned before any links where the package is on the
    /// `from` end.
    ///
    /// If the iteration is in reverse_order, for any given package, at least one link where the
    /// package is on the `from` end is returned before any links where the package is on the `to`
    /// end.
    ///
    /// The default order of iteration is determined by the type of query:
    /// * for `all` and `forward` queries, package IDs are returned in forward order.
    /// * for `reverse` queries, package IDs are returned in reverse order.
    pub fn into_iter_links(self, direction_opt: Option<DependencyDirection>) -> IntoIterLinks<'g> {
        use DependencyDirection::*;

        let direction = direction_opt.unwrap_or_else(|| self.params.default_direction());
        let dep_graph = self.package_graph.dep_graph();

        let (reachable, roots) = select_postfilter(dep_graph, self.params, direction);

        let (reachable, edge_dfs) = match (reachable, direction) {
            (Some(reachable), Forward) => {
                let filtered_graph = NodeFiltered(dep_graph, reachable);
                let edge_dfs = EdgeDfs::new(&filtered_graph, roots);
                (Some(filtered_graph.1), edge_dfs)
            }
            (Some(reachable), Reverse) => {
                let filtered_reversed_graph = NodeFiltered(ReversedDirected(dep_graph), reachable);
                let edge_dfs = EdgeDfs::new(&filtered_reversed_graph, roots);
                (Some(filtered_reversed_graph.1), edge_dfs)
            }
            (None, Forward) => (None, EdgeDfs::new(dep_graph, roots)),
            (None, Reverse) => (None, EdgeDfs::new(ReversedDirected(dep_graph), roots)),
        };

        IntoIterLinks {
            package_graph: self.package_graph,
            reachable,
            edge_dfs,
            direction,
        }
    }
}

/// Computes intermediate state for operations where the graph must be pre-filtered before any
/// traversals happen.
pub(super) fn select_prefilter(
    graph: &Graph<PackageId, DependencyEdge>,
    params: PackageSelectParams,
) -> (FixedBitSet, usize) {
    use PackageSelectParams::*;

    match params {
        All => all_visit_map(graph),
        SelectForward(roots) => reachable_map(graph, roots),
        SelectReverse(roots) => reachable_map(ReversedDirected(graph), roots),
    }
}

/// Computes intermediate state for operations where the graph can be filtered dynamically if
/// possible.
fn select_postfilter(
    graph: &Graph<PackageId, DependencyEdge>,
    params: PackageSelectParams,
    direction: DependencyDirection,
) -> (Option<FixedBitSet>, Vec<NodeIndex<u32>>) {
    use DependencyDirection::*;
    use PackageSelectParams::*;

    match (params, direction) {
        (All, Forward) => {
            // No need for a reachable map, and use all roots.
            let roots: Vec<_> = PackageGraph::roots(graph);
            (None, roots)
        }
        (All, Reverse) => {
            // No need for a reachable map, and use all roots.
            let reversed_graph = ReversedDirected(graph);
            let roots: Vec<_> = PackageGraph::roots(reversed_graph);
            (None, roots)
        }
        (SelectForward(roots), Forward) => {
            // No need for a reachable map.
            (None, roots)
        }
        (SelectForward(roots), Reverse) => {
            // Forward traversal + reverse order = need to compute reachable map.
            let (reachable, _) = reachable_map(graph, roots);
            let filtered_reversed_graph = NodeFiltered(ReversedDirected(graph), reachable);
            // The filtered + reversed graph will have its own roots since the iteration order
            // is reversed from the specified roots.
            let roots: Vec<_> = PackageGraph::roots(&filtered_reversed_graph);

            (Some(filtered_reversed_graph.1), roots)
        }
        (SelectReverse(roots), Forward) => {
            // Reverse traversal + forward order = need to compute reachable map.
            let reversed_graph = ReversedDirected(graph);
            let (reachable, _) = reachable_map(reversed_graph, roots);
            let filtered_graph = NodeFiltered(graph, reachable);
            // The filtered graph will have its own roots since the iteration order is reversed
            // from the specified roots.
            let roots: Vec<_> = PackageGraph::roots(&filtered_graph);

            (Some(filtered_graph.1), roots)
        }
        (SelectReverse(roots), Reverse) => {
            // No need for a reachable map.
            (None, roots)
        }
    }
}

/// An iterator over package IDs in topological order.
///
/// The items returned are of type `&'g PackageId`. Returned by `PackageSelect::into_iter_ids`.
#[derive(Clone, Derivative)]
#[derivative(Debug)]
pub struct IntoIterIds<'g> {
    #[derivative(Debug = "ignore")]
    graph: NodeFiltered<&'g Graph<PackageId, DependencyEdge>, FixedBitSet>,
    // XXX Topo really should implement Debug in petgraph upstream.
    #[derivative(Debug = "ignore")]
    topo: Topo<NodeIndex<u32>, FixedBitSet>,
    direction: DependencyDirection,
    remaining: usize,
}

impl<'g> IntoIterIds<'g> {
    /// Returns the direction the iteration is happening in.
    pub fn direction(&self) -> DependencyDirection {
        self.direction
    }
}

impl<'g> Iterator for IntoIterIds<'g> {
    type Item = &'g PackageId;

    fn next(&mut self) -> Option<Self::Item> {
        let next_idx = match self.direction {
            DependencyDirection::Forward => self.topo.next(&self.graph),
            DependencyDirection::Reverse => self.topo.next(ReversedDirected(&self.graph)),
        };
        next_idx.map(|node_idx| {
            self.remaining -= 1;
            &self.graph.0[node_idx]
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<'g> ExactSizeIterator for IntoIterIds<'g> {
    fn len(&self) -> usize {
        self.remaining
    }
}

/// An iterator over package metadata in topological order.
///
/// The items returned are of type `&'g PackageMetadata`. Returned by `PackageSelect::into_iter_metadatas`.
#[derive(Clone, Debug)]
pub struct IntoIterMetadatas<'g> {
    package_graph: &'g PackageGraph,
    inner: IntoIterIds<'g>,
}

impl<'g> IntoIterMetadatas<'g> {
    /// Returns the direction the iteration is happening in.
    pub fn direction(&self) -> DependencyDirection {
        self.inner.direction
    }
}

impl<'g> Iterator for IntoIterMetadatas<'g> {
    type Item = &'g PackageMetadata;

    fn next(&mut self) -> Option<Self::Item> {
        let next_id = self.inner.next()?;
        Some(
            self.package_graph.metadata(next_id).unwrap_or_else(|| {
                panic!("known package ID '{}' not found in metadata map", next_id)
            }),
        )
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'g> ExactSizeIterator for IntoIterMetadatas<'g> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

/// An iterator over dependency links.
///
/// The items returned are of type `DependencyLink<'g>`. Returned by `PackageSelect::into_iter_ids`.
#[derive(Clone, Derivative)]
#[derivative(Debug)]
pub struct IntoIterLinks<'g> {
    #[derivative(Debug = "ignore")]
    package_graph: &'g PackageGraph,
    reachable: Option<FixedBitSet>,
    edge_dfs: EdgeDfs<EdgeIndex<u32>, NodeIndex<u32>, FixedBitSet>,
    direction: DependencyDirection,
}

impl<'g> IntoIterLinks<'g> {
    /// Returns the direction the iteration is happening in.
    pub fn direction(&self) -> DependencyDirection {
        self.direction
    }

    fn next_triple(&mut self) -> Option<(NodeIndex<u32>, NodeIndex<u32>, EdgeIndex<u32>)> {
        use DependencyDirection::*;

        // This code dynamically switches over all the possible ways to iterate over dependencies.
        // Alternatives would be to either have separate types for all the different sorts of
        // queries (won't get unified type that way) and/or to use a trait object/dynamic iterator
        // (this approach is probably simpler, allocates less, plus there are some lifetime issues
        // with the way petgraph's traits work).
        match (&self.reachable, self.direction) {
            (Some(reachable), Forward) => self.edge_dfs.next(&NodeFiltered::from_fn(
                self.package_graph.dep_graph(),
                |node_idx| reachable.is_visited(&node_idx),
            )),
            (Some(reachable), Reverse) => {
                // As of petgraph 0.4.13, FilterNode is not implemented for &FixedBitSet, only for
                // FixedBitSet. This should be fixable upstream, but use a callback for now.
                // (LLVM should be able to optimize this.)
                self.edge_dfs
                    .next(&NodeFiltered::from_fn(
                        ReversedDirected(self.package_graph.dep_graph()),
                        |node_idx| reachable.is_visited(&node_idx),
                    ))
                    .map(|(source_idx, target_idx, edge_idx)| {
                        // Flip the source and target around if this is a reversed graph, since the
                        // 'from' and 'to' are always right way up.
                        (target_idx, source_idx, edge_idx)
                    })
            }
            (None, Forward) => self.edge_dfs.next(self.package_graph.dep_graph()),
            (None, Reverse) => self
                .edge_dfs
                .next(ReversedDirected(self.package_graph.dep_graph()))
                .map(|(source_idx, target_idx, edge_idx)| {
                    // Flip the source and target around if this is a reversed graph, since the
                    // 'from' and 'to' are always right way up.
                    (target_idx, source_idx, edge_idx)
                }),
        }
    }
}

impl<'g> Iterator for IntoIterLinks<'g> {
    type Item = DependencyLink<'g>;

    fn next(&mut self) -> Option<Self::Item> {
        let next_triple = self.next_triple();

        next_triple.map(|(source_idx, target_idx, edge_idx)| {
            self.package_graph.edge_to_link(
                source_idx,
                target_idx,
                &self.package_graph.dep_graph()[edge_idx],
            )
        })
    }
}

#[derive(Clone, Debug)]
pub(super) enum PackageSelectParams {
    All,
    SelectForward(Vec<NodeIndex<u32>>),
    SelectReverse(Vec<NodeIndex<u32>>),
}

impl PackageSelectParams {
    fn default_direction(&self) -> DependencyDirection {
        match self {
            PackageSelectParams::All | PackageSelectParams::SelectForward(_) => {
                DependencyDirection::Forward
            }
            PackageSelectParams::SelectReverse(_) => DependencyDirection::Reverse,
        }
    }
}

fn all_visit_map<G>(graph: G) -> (FixedBitSet, usize)
where
    G: Visitable<NodeId = NodeIndex<u32>, Map = FixedBitSet>,
{
    let mut visit_map = graph.visit_map();
    // Mark all nodes visited.
    visit_map.insert_range(..);
    let count = visit_map.len();
    (visit_map, count)
}

fn reachable_map<G>(graph: G, roots: Vec<G::NodeId>) -> (FixedBitSet, usize)
where
    G: Visitable<NodeId = NodeIndex<u32>, Map = FixedBitSet> + IntoNeighbors,
{
    // To figure out what nodes are reachable, run a DFS starting from the roots.
    let mut visit_map = graph.visit_map();
    roots.iter().for_each(|node_idx| {
        visit_map.visit(*node_idx);
    });
    let mut dfs = Dfs::from_parts(roots, visit_map);
    while let Some(_) = dfs.next(graph) {}

    // Once the DFS is done, the discovered map is what's reachable.
    let reachable = dfs.discovered;
    let count = reachable.count_ones(..);
    (reachable, count)
}
