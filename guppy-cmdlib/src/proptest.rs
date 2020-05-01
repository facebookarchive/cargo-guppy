// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Proptest support.

use crate::PackagesAndFeatures;
use guppy::graph::PackageGraph;
use proptest::collection::hash_set;
use proptest::prelude::*;

impl PackagesAndFeatures {
    pub fn strategy<'g>(graph: &'g PackageGraph) -> impl Strategy<Value = Self> + 'g {
        let workspace = graph.workspace();
        (
            // The lower bound of 0 is important because 0 means the whole workspace.
            hash_set(workspace.prop09_name_strategy(), 0..8),
            any::<bool>(),
            any::<bool>(),
        )
            .prop_map(move |(packages, all_features, no_default_features)| {
                // TODO: select features from these packages (probably requires flat_map :/ )
                Self {
                    packages: packages
                        .into_iter()
                        .map(|package| package.to_string())
                        .collect(),
                    features: vec![],
                    all_features,
                    no_default_features,
                }
            })
    }
}
