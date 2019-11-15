use crate::errors::*;
use crate::graph::visit::reversed::ReversedDirected;
use crate::graph::{DependencyDirection, DependencyEdge, PackageGraph};
use cargo_metadata::PackageId;
use fixedbitset::FixedBitSet;
use petgraph::prelude::*;
use petgraph::visit::{IntoNeighbors, NodeFiltered, Topo, VisitMap, Visitable};

/// A selector over a package graph.
///
/// This is the entry point for iterators over IDs and dependency links, and dot graph presentation.
/// A `PackageSelect` is constructed through the `select_` methods on `PackageGraph`.
#[derive(Clone, Debug)]
pub struct PackageSelect<'g> {
    package_graph: &'g PackageGraph,
    params: PackageSelectParams,
}

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
    pub fn select_transitive_deps_directed<'g, 'a>(
        &'g self,
        package_ids: impl IntoIterator<Item = &'a PackageId>,
        dep_direction: DependencyDirection,
    ) -> Result<PackageSelect<'g>, Error> {
        match dep_direction {
            DependencyDirection::Forward => self.select_transitive_deps(package_ids),
            DependencyDirection::Reverse => self.select_transitive_reverse_deps(package_ids),
        }
    }

    /// Creates a new selector that returns transitive dependencies of the given packages.
    ///
    /// Returns an error if any package IDs are unknown.
    pub fn select_transitive_deps<'g, 'a>(
        &'g self,
        package_ids: impl IntoIterator<Item = &'a PackageId>,
    ) -> Result<PackageSelect<'g>, Error> {
        Ok(PackageSelect {
            package_graph: self,
            params: PackageSelectParams::TransitiveDeps(self.node_idxs(package_ids)?),
        })
    }

    /// Creates a new selector that returns transitive reverse dependencies of the given packages.
    ///
    /// Returns an error if any package IDs are unknown.
    pub fn select_transitive_reverse_deps<'g, 'a>(
        &'g self,
        package_ids: impl IntoIterator<Item = &'a PackageId>,
    ) -> Result<PackageSelect<'g>, Error> {
        Ok(PackageSelect {
            package_graph: self,
            params: PackageSelectParams::TransitiveReverseDeps(self.node_idxs(package_ids)?),
        })
    }
}

impl<'g> PackageSelect<'g> {
    /// Consumes this query and creates an iterator over package IDs, returned in topological order.
    ///
    /// The default order of iteration is determined by the type of query:
    /// * for `all` and `transitive_deps` queries, package IDs are returned in forward order.
    /// * for `transitive_reverse_deps` queries, package IDs are returned in reverse order.
    pub fn into_iter_ids(self, direction_opt: Option<DependencyDirection>) -> PackageIdIter<'g> {
        let direction = direction_opt.unwrap_or_else(|| self.params.default_direction());
        let dep_graph = &self.package_graph.dep_graph;

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
        PackageIdIter {
            graph: filtered_graph,
            topo,
            direction,
            remaining: count,
        }
    }
}

/// Computes intermediate state for operations where the graph must be pre-filtered before any
/// traversals happen.
fn select_prefilter(
    graph: &Graph<PackageId, DependencyEdge>,
    params: PackageSelectParams,
) -> (FixedBitSet, usize) {
    use PackageSelectParams::*;

    match params {
        All => all_visit_map(graph),
        TransitiveDeps(roots) => reachable_map(graph, roots),
        TransitiveReverseDeps(roots) => reachable_map(ReversedDirected(graph), roots),
    }
}

/// An iterator over package IDs in topological order.
///
/// The items returned are of type `&'g PackageId`. Returned by `PackageSelect::into_iter_ids`.
#[derive(Clone)]
pub struct PackageIdIter<'g> {
    graph: NodeFiltered<&'g Graph<PackageId, DependencyEdge>, FixedBitSet>,
    topo: Topo<NodeIndex<u32>, FixedBitSet>,
    direction: DependencyDirection,
    remaining: usize,
}

impl<'g> PackageIdIter<'g> {
    /// Returns the direction the iteration is happening in.
    pub fn direction(&self) -> DependencyDirection {
        self.direction
    }
}

impl<'g> Iterator for PackageIdIter<'g> {
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

impl<'g> ExactSizeIterator for PackageIdIter<'g> {
    fn len(&self) -> usize {
        self.remaining
    }
}

#[derive(Clone, Debug)]
enum PackageSelectParams {
    All,
    TransitiveDeps(Vec<NodeIndex<u32>>),
    TransitiveReverseDeps(Vec<NodeIndex<u32>>),
}

impl PackageSelectParams {
    fn default_direction(&self) -> DependencyDirection {
        match self {
            PackageSelectParams::All | PackageSelectParams::TransitiveDeps(_) => {
                DependencyDirection::Forward
            }
            PackageSelectParams::TransitiveReverseDeps(_) => DependencyDirection::Reverse,
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
