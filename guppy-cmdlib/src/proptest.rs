// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Proptest support.

use crate::PackagesAndFeatures;
use guppy::{graph::PackageGraph, Platform, TargetFeatures};
use proptest::{collection::hash_set, prelude::*};

impl PackagesAndFeatures {
    pub fn strategy<'g>(graph: &'g PackageGraph) -> impl Strategy<Value = Self> + 'g {
        let workspace = graph.workspace();
        (
            // The lower bound of 0 is important because 0 means the whole workspace.
            hash_set(workspace.prop010_name_strategy(), 0..8),
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

/// Generates a random, known target triple that can be understood by both cargo and guppy, or
/// `None`.
pub fn triple_strategy() -> impl Strategy<Value = Option<&'static str>> {
    let platform_strategy = Platform::filtered_strategy(
        |triple| {
            // Filter out Apple platforms because rustc requires the Apple SDKs to be set up for
            // them.
            !triple.contains("-apple-")
        },
        Just(TargetFeatures::Unknown),
    );
    prop_oneof![
        // 25% chance to generate None, 75% to generate a particular platform
        1 => Just(None),
        3 => platform_strategy.prop_map(|platform| Some(platform.triple())),
    ]
}
