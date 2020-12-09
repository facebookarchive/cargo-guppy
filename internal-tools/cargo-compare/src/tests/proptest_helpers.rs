// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::common::GuppyCargoCommon;
use crate::diff::DiffOpts;
use crate::GlobalContext;
use guppy::graph::PackageGraph;
use proptest::test_runner::{TestCaseError, TestCaseResult};
use std::env;

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
                    compare(fixture.graph(), common)?;
                });
            }
        }
    }
}

/// Test that there is no diff between guppy and cargo for the same query.
pub(super) fn compare(graph: &PackageGraph, common: GuppyCargoCommon) -> TestCaseResult {
    let verbose = matches!(
        env::var("PROPTEST_VERBOSE")
            .as_ref()
            .map(|val| val.as_str()),
        Ok("true") | Ok("1")
    );
    let diff_opts = DiffOpts { common, verbose };
    let ctx = GlobalContext::new(true, graph).expect("context created");
    let target_host_diff = diff_opts
        .compute_diff(&ctx)
        .expect("compute_diff succeeded");
    if target_host_diff.any_diff() {
        println!("{}", target_host_diff);
        Err(TestCaseError::fail("diff found"))
    } else {
        Ok(())
    }
}
