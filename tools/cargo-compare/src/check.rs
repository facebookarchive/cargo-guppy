// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::common::GuppyCargoCommon;
use crate::diff::{FeatureDiff, TargetHostDiff};
use crate::GlobalContext;
use anyhow::{Error, Result};
use guppy_cmdlib::CargoMetadataOptions;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestError, TestRunner};
use std::sync::atomic::{AtomicUsize, Ordering};
use structopt::StructOpt;

/// Options for cargo/guppy comparisons.
#[derive(Debug, StructOpt)]
pub struct CheckOpts {
    /// Number of randomly generated diff operations to run
    #[structopt(long, default_value = "256")]
    pub cases: u32,
    /// Print a message every n test cases. Use '0' to disable
    #[structopt(long, default_value = "16")]
    pub print_every: usize,
    #[structopt(flatten)]
    pub metadata: CargoMetadataOptions,
    /// Print out unchanged packages and features as well
    #[structopt(long)]
    pub verbose: bool,
}

impl CheckOpts {
    /// Executes this command.
    pub fn exec(self, ctx: &GlobalContext) -> Result<()> {
        let strat = GuppyCargoCommon::strategy(&self.metadata, ctx.graph);

        let mut testrunner = TestRunner::new(Config {
            cases: self.cases,
            ..Config::default()
        });

        // print a message after every n tests
        let test_count = AtomicUsize::new(0);

        testrunner
            .run(&strat, |common| {
                let cargo_map = common
                    .resolve_cargo(ctx)
                    .map_err(|e| TestCaseError::fail(e.to_string()))?;
                let guppy_map = common
                    .resolve_guppy(ctx)
                    .map_err(|e| TestCaseError::fail(e.to_string()))?;

                let target_diff = FeatureDiff {
                    graph: ctx.graph(),
                    a: guppy_map.target_map,
                    b: cargo_map.target_map,
                    verbose: self.verbose,
                };

                let host_diff = FeatureDiff {
                    graph: ctx.graph(),
                    a: guppy_map.host_map,
                    b: cargo_map.host_map,
                    verbose: self.verbose,
                };

                let diff = TargetHostDiff::new(target_diff, host_diff);

                if self.print_every != 0 {
                    let test_count = test_count.fetch_add(1, Ordering::SeqCst);
                    if test_count % self.print_every == 0 && test_count != 0 {
                        println!("finished running {} tests", test_count);
                    };
                };

                prop_assert!(!diff.any_diff(), "unexpected diff: {}", diff);

                Ok(())
            })
            .map_err(|e| match e {
                TestError::Abort(e) => {
                    Error::msg(format!("Aborted cargo/guppy diff check, {:?}", e))
                }
                TestError::Fail(e, v) => {
                    Error::msg(format!("Failed cargo/guppy diff check {:?}\n{:?}", e, v))
                }
            })
    }
}
