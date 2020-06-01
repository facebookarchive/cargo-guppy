// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::common::GuppyCargoCommon;
use crate::diff::DiffOpts;
use crate::GlobalContext;
use guppy::graph::PackageGraph;

macro_rules! proptest_suite {
    ($name: ident) => {
        mod $name {
            use crate::tests::fixtures::Fixture;
            use crate::tests::proptest_helpers::*;
            use proptest::prelude::*;

            #[test]
            fn proptest_compare() {
                let fixture = Fixture::$name();
                // cargo is pretty slow, so limit the number of test cases.
                proptest!(ProptestConfig::with_cases(fixture.num_proptests()), |(
                    common in fixture.common_strategy(),
                )| {
                    compare(fixture.graph(), common);
                });
            }
        }
    }
}

/// Test that there is no diff between guppy and cargo for the same query.
pub(super) fn compare(graph: &PackageGraph, common: GuppyCargoCommon) {
    let diff_opts = DiffOpts {
        common,
        verbose: false,
    };
    let ctx = GlobalContext::new(true, graph).expect("context created");
    diff_opts.exec(&ctx).expect("no errors and no diff found");
}
