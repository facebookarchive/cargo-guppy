use crate::errors::*;
use crate::graph::visit::dot::{DotFmt, DotVisitor, DotWrite};
use crate::graph::visit::reversed::{ReverseFlip, ReversedDirected};
use crate::graph::{DependencyEdge, DependencyLink, PackageGraph, PackageMetadata};
use cargo_metadata::PackageId;
use petgraph::prelude::*;
use petgraph::visit::{IntoNeighbors, NodeFiltered, NodeRef, VisitMap, Visitable};
use std::fmt;

/// A visitor used for formatting `dot` graphs.
pub trait PackageDotVisitor {
    /// Visits this package. The implementation may output a label for this package to the given
    /// `DotWrite`.
    fn visit_package(&self, package: &PackageMetadata, f: DotWrite<'_, '_>) -> fmt::Result;

    /// Visits this dependency link. The implementation may output a label for this link to the
    /// given `DotWrite`.
    fn visit_link(&self, link: DependencyLink<'_>, f: DotWrite<'_, '_>) -> fmt::Result;
}

impl PackageGraph {
    /// Constructs a representation of this graph in `dot` format.
    pub fn make_dot<'g, V>(&'g self, visitor: V) -> impl fmt::Display + 'g
    where
        V: 'g + PackageDotVisitor,
    {
        DotFmt::new(&self.dep_graph, VisitorWrap::new(self, visitor))
    }

    /// Constructs a representation of all packages that are dependencies of the given roots, in
    /// `dot` format.
    pub fn make_dot_reachable<'g, 'a, V>(
        &'g self,
        roots: impl IntoIterator<Item = &'a PackageId>,
        visitor: V,
    ) -> Result<impl fmt::Display + 'g, Error>
    where
        V: 'g + PackageDotVisitor,
    {
        let node_idxs = self.node_idxs(roots)?;
        let node_filtered =
            NodeFiltered(&self.dep_graph, reachable_map(&self.dep_graph, node_idxs));
        Ok(DotFmt::new(node_filtered, VisitorWrap::new(self, visitor)))
    }

    /// Constructs a representation of all packages that are dependents of the given roots, in
    /// `dot` format.
    pub fn make_dot_reachable_reversed<'g, 'a, V>(
        &'g self,
        roots: impl IntoIterator<Item = &'a PackageId>,
        visitor: V,
    ) -> Result<impl fmt::Display + 'g, Error>
    where
        V: 'g + PackageDotVisitor,
    {
        let node_idxs = self.node_idxs(roots)?;
        // The reachable_map is computed over the reversed graph, while the actual iteration happens
        // over the regular graph (so that arrows in the graph are in the right direction).
        let node_filtered = NodeFiltered(
            &self.dep_graph,
            reachable_map(ReversedDirected(&self.dep_graph), node_idxs),
        );
        Ok(DotFmt::new(node_filtered, VisitorWrap::new(self, visitor)))
    }
}

struct VisitorWrap<'g, V> {
    graph: &'g PackageGraph,
    inner: V,
}

impl<'g, V> VisitorWrap<'g, V> {
    fn new(graph: &'g PackageGraph, inner: V) -> Self {
        Self { graph, inner }
    }
}

impl<'g, V, NR, ER> DotVisitor<NR, ER> for VisitorWrap<'g, V>
where
    V: PackageDotVisitor,
    NR: NodeRef<NodeId = NodeIndex<u32>, Weight = PackageId>,
    ER: EdgeRef<NodeId = NodeIndex<u32>, Weight = DependencyEdge> + ReverseFlip,
{
    fn visit_node(&self, node: NR, f: DotWrite<'_, '_>) -> fmt::Result {
        let metadata = self
            .graph
            .metadata(node.weight())
            .expect("visited node should have associated metadata");
        self.inner.visit_package(metadata, f)
    }

    fn visit_edge(&self, edge: ER, f: DotWrite<'_, '_>) -> fmt::Result {
        let (source_idx, target_idx) = ER::reverse_flip(edge.source(), edge.target());
        let link = self
            .graph
            .edge_to_link(source_idx, target_idx, edge.weight());
        self.inner.visit_link(link, f)
    }
}

fn reachable_map<G>(graph: G, roots: Vec<G::NodeId>) -> G::Map
where
    G: Visitable + IntoNeighbors,
{
    // To figure out what nodes are reachable, run a DFS starting from the roots.
    let mut visit_map = graph.visit_map();
    roots.iter().for_each(|node_idx| {
        visit_map.visit(*node_idx);
    });
    let mut dfs = Dfs::from_parts(roots, visit_map);
    while let Some(_) = dfs.next(graph) {}

    // Once the DFS is done, the discovered map is what's reachable.
    dfs.discovered
}
