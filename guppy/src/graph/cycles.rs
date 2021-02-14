// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Code for handling cycles in dependency graphs.

use crate::{
    graph::{PackageGraph, PackageIx},
    petgraph_support::scc::Sccs,
    Error, PackageId,
};

/// Contains information about dependency cycles.
///
/// Cargo permits cycles if at least one of the links is dev-only. `Cycles` exposes information
/// about such dependencies.
///
/// Constructed through `PackageGraph::cycles`.
pub struct Cycles<'g> {
    package_graph: &'g PackageGraph,
    sccs: &'g Sccs<PackageIx>,
}

impl<'g> Cycles<'g> {
    pub(super) fn new(package_graph: &'g PackageGraph) -> Self {
        Self {
            package_graph,
            sccs: package_graph.sccs(),
        }
    }

    /// Returns true if these two IDs are in the same cycle.
    pub fn is_cyclic(&self, a: &PackageId, b: &PackageId) -> Result<bool, Error> {
        let a_ix = self.package_graph.package_ix(a)?;
        let b_ix = self.package_graph.package_ix(b)?;
        Ok(self.sccs.is_same_scc(a_ix, b_ix))
    }

    /// Returns all the cycles of 2 or more elements in this graph.
    ///
    /// Cycles are returned in topological order: if packages in cycle B depend on packages in cycle
    /// A, A is returned before B.
    ///
    /// Within a cycle, nodes are returned in non-dev order: if package Foo has a dependency on Bar,
    /// and Bar has a cyclic dev-dependency on Foo, then Foo is returned before Bar.
    pub fn all_cycles(
        &self,
    ) -> impl Iterator<Item = Vec<&'g PackageId>> + DoubleEndedIterator + 'g {
        let dep_graph = &self.package_graph.dep_graph;
        self.sccs
            .multi_sccs()
            .map(move |scc| scc.iter().map(move |ix| &dep_graph[*ix]).collect())
    }
}
