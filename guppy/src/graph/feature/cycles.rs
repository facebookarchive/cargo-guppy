// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Code for handling cycles in feature graphs.

use crate::{
    graph::{
        feature::{FeatureGraph, FeatureId},
        FeatureIx,
    },
    petgraph_support::scc::Sccs,
    Error,
};

/// Contains information about dependency cycles in feature graphs.
///
/// Cargo permits cycles if at least one of the links is dev-only. `Cycles` exposes information
/// about such dependencies.
///
/// Constructed through `PackageGraph::cycles`.
pub struct Cycles<'g> {
    feature_graph: FeatureGraph<'g>,
    sccs: &'g Sccs<FeatureIx>,
}

impl<'g> Cycles<'g> {
    pub(super) fn new(feature_graph: FeatureGraph<'g>) -> Self {
        Self {
            feature_graph,
            sccs: feature_graph.sccs(),
        }
    }

    /// Returns true if these two IDs are in the same cycle.
    pub fn is_cyclic<'a>(
        &self,
        a: impl Into<FeatureId<'a>>,
        b: impl Into<FeatureId<'a>>,
    ) -> Result<bool, Error> {
        let a = a.into();
        let b = b.into();
        let a_ix = self.feature_graph.feature_ix(a)?;
        let b_ix = self.feature_graph.feature_ix(b)?;
        Ok(self.sccs.is_same_scc(a_ix, b_ix))
    }

    /// Returns all the cycles of 2 or more elements in this graph.
    ///
    /// The order returned within each cycle is arbitrary.
    pub fn all_cycles(&self) -> impl Iterator<Item = Vec<FeatureId<'g>>> + 'g {
        let dep_graph = self.feature_graph.dep_graph();
        let package_graph = self.feature_graph.package_graph;
        self.sccs.multi_sccs().map(move |class| {
            class
                .iter()
                .map(move |feature_ix| FeatureId::from_node(package_graph, &dep_graph[*feature_ix]))
                .collect()
        })
    }
}
