use crate::errors::Error;
use crate::graph::{DependencyEdge, PackageDep, PackageGraph};
use cargo_metadata::PackageId;
use petgraph::graph::EdgeReference;
use petgraph::prelude::*;
use petgraph::visit::{EdgeFiltered, FilterEdge, Visitable, Walker};

pub struct EdgeFilteredPackageGraph<'g, F> {
    original: &'g PackageGraph,
    filtered: EdgeFiltered<&'g Graph<PackageId, DependencyEdge>, FilterPackageDep<'g, F>>,
}

impl<'g, F> EdgeFilteredPackageGraph<'g, F>
where
    F: Fn(PackageDep<'_>) -> bool,
{
    pub fn new(original: &'g PackageGraph, filter: F) -> Self {
        let filtered = EdgeFiltered(
            original.dep_graph(),
            FilterPackageDep::new(original, filter),
        );
        Self { original, filtered }
    }

    /// Returns all transitive dependencies for the given package IDs.
    pub fn transitive_deps<'a, 'b>(
        &'a self,
        package_ids: impl IntoIterator<Item = &'b PackageId>,
    ) -> Result<impl Iterator<Item = &'g PackageId> + 'a, Error> {
        let node_idxs = self.original.node_idxs(package_ids)?;

        let bfs = Bfs {
            stack: node_idxs,
            discovered: self.filtered.visit_map(),
        };

        let dep_graph = self.original.dep_graph();
        Ok(bfs.iter(&self.filtered).map(move |node_idx| {
            // Each node_idx in the filtered graph is also in the original.
            &dep_graph[node_idx]
        }))
    }
}

pub struct FilterPackageDep<'g, F> {
    original: &'g PackageGraph,
    filter: F,
}

impl<'g, F> FilterPackageDep<'g, F> {
    fn new(original: &'g PackageGraph, filter: F) -> Self {
        Self { original, filter }
    }
}

// (sorry about spelling the types out -- maybe type equality constraints can make this simpler?)
impl<'g, F> FilterEdge<EdgeReference<'g, DependencyEdge, u32>> for FilterPackageDep<'g, F>
where
    F: Fn(PackageDep<'_>) -> bool,
{
    fn include_edge(&self, edge: EdgeReference<'g, DependencyEdge, u32>) -> bool {
        (self.filter)(
            self.original
                .edge_to_dep(edge.source(), edge.target(), edge.weight()),
        )
    }
}
