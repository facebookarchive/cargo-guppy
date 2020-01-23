// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::{DependencyDirection, DependencyLink, PackageGraph};
use crate::PackageId;
use petgraph::prelude::*;
use proptest09::prelude::*;

/// ## Helpers for property testing
///
/// The methods in this section allow a `PackageGraph` to be used in property-based testing
/// scenarios.
///
/// Currently, [proptest 0.9](https://docs.rs/proptest/0.9) is supported if the `proptest09` feature
/// is enabled.
impl PackageGraph {
    /// Returns a `Strategy` that generates random package IDs from this graph.
    ///
    /// Requires the `proptest09` feature to be enabled.
    ///
    /// ## Panics
    ///
    /// Panics if there are no packages in this `PackageGraph`.
    pub fn prop09_id_strategy<'g>(&'g self) -> impl Strategy<Value = &'g PackageId> + 'g {
        let dep_graph = &self.dep_graph;
        any::<prop::sample::Index>().prop_map(move |index| {
            let package_ix = NodeIndex::new(index.index(dep_graph.node_count()));
            &self.dep_graph[package_ix]
        })
    }

    /// Returns a `Strategy` that generates random dependency links from this graph.
    ///
    /// Requires the `proptest09` feature to be enabled.
    ///
    /// ## Panics
    ///
    /// Panics if there are no dependency edges in this `PackageGraph`.
    pub fn prop09_link_strategy<'g>(&'g self) -> impl Strategy<Value = DependencyLink<'g>> + 'g {
        any::<prop::sample::Index>().prop_map(move |index| {
            // Note that this works because PackageGraph uses petgraph::Graph, not StableGraph. If
            // PackageGraph used StableGraph, a retain_edges call would create holes -- invalid
            // indexes in the middle of the range. Graph compacts edge indexes so that all
            // indexes from 0 to link_count are valid.
            let edge_idx = EdgeIndex::new(index.index(self.link_count()));
            let (source_idx, target_idx) = self
                .dep_graph
                .edge_endpoints(edge_idx)
                .expect("all edge indexes 0..link_count should be valid");
            self.edge_to_link(source_idx, target_idx, &self.dep_graph[edge_idx])
        })
    }
}

impl Arbitrary for DependencyDirection {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_params: Self::Parameters) -> Self::Strategy {
        prop_oneof![
            Just(DependencyDirection::Forward),
            Just(DependencyDirection::Reverse)
        ]
        .boxed()
    }
}
