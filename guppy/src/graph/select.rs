// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::debug_ignore::DebugIgnore;
use crate::graph::select_core::{select_postfilter, SelectParams, SelectPrefilter};
use crate::graph::{
    DependencyDirection, DependencyEdge, DependencyLink, PackageGraph, PackageIx, PackageMetadata,
};
use crate::petgraph_support::scc::NodeIter;
use crate::petgraph_support::walk::EdgeDfs;
use crate::{Error, PackageId};
use fixedbitset::FixedBitSet;
use petgraph::prelude::*;
use petgraph::visit::{NodeFiltered, Reversed, VisitMap};

/// A selector over a package graph.
///
/// This is the entry point for iterators over IDs and dependency links, and dot graph presentation.
/// A `PackageSelect` is constructed through the `select_` methods on `PackageGraph`.
#[derive(Clone, Debug)]
pub struct PackageSelect<'g> {
    // The fields are pub(super) for access within the graph module.
    pub(super) package_graph: &'g PackageGraph,
    pub(super) params: SelectParams<PackageGraph>,
}

/// ## Selectors
///
/// The methods in this section create *package selectors*, which are queries over subsets of this
/// package graph. Use the methods here for queries based on transitive dependencies.
impl PackageGraph {
    /// Creates a new forward selector over the entire workspace.
    ///
    /// `select_workspace` will select all workspace packages and their transitive dependencies.
    pub fn select_workspace(&self) -> PackageSelect {
        self.select_forward(self.workspace().member_ids())
            .expect("workspace packages should all be known")
    }

    /// Creates a new selector that returns all members of this package graph.
    ///
    /// This is normally the same as `select_workspace`, but can differ in some cases:
    /// * if packages have been replaced with `[patch]` or `[replace]`
    /// * if some edges have been removed from this graph with `retain_edges`.
    ///
    /// In most situations, `select_workspace` is preferred. Use `select_all` if you know you need
    /// parts of the graph that aren't accessible from the workspace.
    pub fn select_all(&self) -> PackageSelect {
        PackageSelect {
            package_graph: self,
            params: SelectParams::All,
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
            params: SelectParams::SelectForward(self.package_ixs(package_ids)?),
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
            params: SelectParams::SelectReverse(self.package_ixs(package_ids)?),
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
    ///
    /// ## Cycles
    ///
    /// If a root consists of a dependency cycle, all the packages in it will be returned in
    /// arbitrary order.
    pub fn into_root_ids(
        self,
        direction: DependencyDirection,
    ) -> impl Iterator<Item = &'g PackageId> + ExactSizeIterator + 'g {
        let dep_graph = self.package_graph.dep_graph();
        let prefilter = SelectPrefilter::new(dep_graph, self.params);
        prefilter
            .roots(self.package_graph.sccs(), direction)
            .into_iter()
            .map(move |package_ix| &dep_graph[package_ix])
    }

    /// Consumes this query and creates an iterator over package IDs, returned in topological order.
    ///
    /// The default order of iteration is determined by the type of query:
    /// * for `all` and `forward` queries, package IDs are returned in forward order.
    /// * for `reverse` queries, package IDs are returned in reverse order.
    ///
    /// ## Cycles
    ///
    /// The packages within a dependency cycle will be returned in arbitrary order, but overall
    /// topological order will be maintained.
    pub fn into_iter_ids(self, direction_opt: Option<DependencyDirection>) -> IntoIterIds<'g> {
        let direction = direction_opt.unwrap_or_else(|| self.params.default_direction());
        let dep_graph = self.package_graph.dep_graph();
        let sccs = self.package_graph.sccs();

        // If the topo order guarantee weren't present, this could potentially be done in a lazier
        // fashion, where reachable nodes are discovered dynamically. However, there's value in
        // topological ordering so pay the cost of computing the reachable map upfront. As a bonus,
        // this approach also allows the iterator to implement ExactSizeIterator.
        let prefilter = SelectPrefilter::new(dep_graph, self.params);

        // ---
        // IMPORTANT
        // ---
        //
        // This uses the same list of sccs that's computed for the entire graph. This is *currently*
        // fine because with our current filters, if one element of an SCC is present all others
        // will be present as well.
        //
        // However:
        // * If we allow for custom visitors that can control the reachable map, it is possible that
        //   SCCs in the main graph aren't in the subgraph. That might make the returned order
        //   incorrect.
        // * This requires iterating over every node in the graph even if the set of returned nodes
        //   is very small. There's a tradeoff here between allocating memory to store a custom list
        //   of SCCs and just using the one available. More benchmarking is required to figure out
        //   the best approach.
        //
        // Note that the SCCs can be computed in reachable_map by adapting parts of kosaraju_scc.
        let node_iter = sccs.node_iter(direction.into());

        IntoIterIds {
            graph: DebugIgnore(dep_graph),
            node_iter,
            reachable: prefilter.reachable,
            remaining: prefilter.count,
        }
    }

    /// Returns the set of "root package" metadata in the specified direction.
    ///
    /// * If direction is Forward, return the set of metadatas that do not have any dependencies
    ///   within the selected graph.
    /// * If direction is Reverse, return the set of metadatas that do not have any dependents within
    ///   the selected graph.
    ///
    /// ## Cycles
    ///
    /// If a root consists of a dependency cycle, all the packages in it will be returned in
    /// arbitrary order.
    pub fn into_root_metadatas(
        self,
        direction: DependencyDirection,
    ) -> impl Iterator<Item = &'g PackageMetadata> + ExactSizeIterator + 'g {
        let package_graph = self.package_graph;
        let dep_graph = package_graph.dep_graph();
        let prefilter = SelectPrefilter::new(dep_graph, self.params);
        prefilter
            .roots(package_graph.sccs(), direction)
            .into_iter()
            .map(move |package_ix| {
                package_graph
                    .metadata(&dep_graph[package_ix])
                    .expect("invalid node index")
            })
    }

    /// Consumes this query and creates an iterator over `PackageMetadata` instances, returned in
    /// topological order.
    ///
    /// The default order of iteration is determined by the type of query:
    /// * for `all` and `forward` queries, package IDs are returned in forward order.
    /// * for `reverse` queries, package IDs are returned in reverse order.
    ///
    /// ## Cycles
    ///
    /// The packages within a dependency cycle will be returned in arbitrary order, but overall
    /// topological order will be maintained.
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

        let (reachable, initials) =
            select_postfilter(dep_graph, self.params, self.package_graph.sccs(), direction);

        let (reachable, edge_dfs) = match (reachable, direction) {
            (Some(reachable), Forward) => {
                let filtered_graph = NodeFiltered(dep_graph, reachable);
                let edge_dfs = EdgeDfs::new(&filtered_graph, initials);
                (Some(filtered_graph.1), edge_dfs)
            }
            (Some(reachable), Reverse) => {
                let filtered_reversed_graph = NodeFiltered(Reversed(dep_graph), reachable);
                let edge_dfs = EdgeDfs::new(&filtered_reversed_graph, initials);
                (Some(filtered_reversed_graph.1), edge_dfs)
            }
            (None, Forward) => (None, EdgeDfs::new(dep_graph, initials)),
            (None, Reverse) => (None, EdgeDfs::new(Reversed(dep_graph), initials)),
        };

        IntoIterLinks {
            package_graph: DebugIgnore(self.package_graph),
            reachable,
            edge_dfs,
            direction,
        }
    }
}
/// An iterator over package IDs in topological order.
///
/// The items returned are of type `&'g PackageId`. Returned by `PackageSelect::into_iter_ids`.
#[derive(Clone, Debug)]
pub struct IntoIterIds<'g> {
    graph: DebugIgnore<&'g Graph<PackageId, DependencyEdge, Directed, PackageIx>>,
    node_iter: NodeIter<'g, PackageIx>,
    reachable: FixedBitSet,
    remaining: usize,
}

impl<'g> IntoIterIds<'g> {
    /// Returns the direction the iteration is happening in.
    pub fn direction(&self) -> DependencyDirection {
        self.node_iter.direction().into()
    }
}

impl<'g> Iterator for IntoIterIds<'g> {
    type Item = &'g PackageId;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(ix) = self.node_iter.next() {
            if !self.reachable.is_visited(&ix) {
                continue;
            }
            self.remaining -= 1;
            return Some(&self.graph[ix]);
        }
        None
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
        self.inner.direction()
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
#[derive(Clone, Debug)]
pub struct IntoIterLinks<'g> {
    package_graph: DebugIgnore<&'g PackageGraph>,
    reachable: Option<FixedBitSet>,
    edge_dfs: EdgeDfs<EdgeIndex<PackageIx>, NodeIndex<PackageIx>, FixedBitSet>,
    direction: DependencyDirection,
}

impl<'g> IntoIterLinks<'g> {
    /// Returns the direction the iteration is happening in.
    pub fn direction(&self) -> DependencyDirection {
        self.direction
    }

    fn next_triple(
        &mut self,
    ) -> Option<(
        NodeIndex<PackageIx>,
        NodeIndex<PackageIx>,
        EdgeIndex<PackageIx>,
    )> {
        use DependencyDirection::*;

        // This code dynamically switches over all the possible ways to iterate over dependencies.
        // Alternatives would be to either have separate types for all the different sorts of
        // queries (won't get unified type that way) and/or to use a trait object/dynamic iterator
        // (this approach is probably simpler, allocates less, plus there are some lifetime issues
        // with the way petgraph's traits work).
        match (&self.reachable, self.direction) {
            (Some(reachable), Forward) => self.edge_dfs.next(&NodeFiltered::from_fn(
                self.package_graph.dep_graph(),
                |ix| reachable.is_visited(&ix),
            )),
            (Some(reachable), Reverse) => {
                // As of petgraph 0.4.13, FilterNode is not implemented for &FixedBitSet, only for
                // FixedBitSet. This should be fixable upstream, but use a callback for now.
                // (LLVM should be able to optimize this.)
                self.edge_dfs
                    .next(&NodeFiltered::from_fn(
                        Reversed(self.package_graph.dep_graph()),
                        |ix| reachable.is_visited(&ix),
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
                .next(Reversed(self.package_graph.dep_graph()))
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
