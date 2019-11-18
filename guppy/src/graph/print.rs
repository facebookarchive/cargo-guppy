// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::select::select_prefilter;
use crate::graph::{DependencyEdge, DependencyLink, PackageGraph, PackageMetadata, PackageSelect};
use crate::petgraph_support::dot::{DotFmt, DotVisitor, DotWrite};
use crate::petgraph_support::reversed::ReverseFlip;
use cargo_metadata::PackageId;
use petgraph::prelude::*;
use petgraph::visit::{NodeFiltered, NodeRef};
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

impl<'g> PackageSelect<'g> {
    /// Constructs a representation of the selected graph in `dot` format.
    pub fn into_dot<V>(self, visitor: V) -> impl fmt::Display + 'g
    where
        V: 'g + PackageDotVisitor,
    {
        // dot graphs are always forward iterated, and prefiltering is necessary in order to
        // figure out which nodes should be included.
        let dep_graph = self.package_graph.dep_graph();
        let (reachable, _) = select_prefilter(dep_graph, self.params);
        let node_filtered = NodeFiltered(dep_graph, reachable);
        DotFmt::new(node_filtered, VisitorWrap::new(self.package_graph, visitor))
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
