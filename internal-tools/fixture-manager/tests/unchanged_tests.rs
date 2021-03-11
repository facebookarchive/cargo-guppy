// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{bail, Result};
use fixture_manager::{
    context::GenerateContext, hakari_toml::HakariTomlContext, summaries::SummaryContext,
    GenerateHakariOpts, GenerateSummariesOpts,
};
use fixtures::json::JsonFixture;

/// Test that no checked in summaries have changed.
#[test]
fn summaries_unchanged() -> Result<()> {
    let mut num_changed = 0;

    for (name, fixture) in JsonFixture::all_fixtures() {
        let count = GenerateSummariesOpts::default_count();

        println!("generating {} summaries for {}...", count, name);

        let context: GenerateContext<'_, SummaryContext> =
            GenerateContext::new(fixture, &count, false)?;

        for item in context {
            let item = item?;
            let is_changed = item.is_changed();
            if is_changed {
                num_changed += 1;
                println!("** {}:\n{}", item.path(), item.diff());
            }
        }
    }

    if num_changed > 0 {
        bail!("{} summaries changed", num_changed);
    }

    Ok(())
}

/// Test that no checked in Hakari files have changed.
#[test]
fn hakari_unchanged() -> Result<()> {
    let mut num_changed = 0;

    for (name, fixture) in JsonFixture::all_fixtures() {
        let count = GenerateHakariOpts::default_count();

        println!("generating {} outputs for {}...", count, name);

        let context: GenerateContext<'_, HakariTomlContext> =
            GenerateContext::new(fixture, &GenerateHakariOpts::default_count(), false)?;

        for item in context {
            let item = item?;
            let is_changed = item.is_changed();
            if is_changed {
                num_changed += 1;
                println!("** (fixture {}) {}:\n{}", name, item.path(), item.diff());
            }
        }
    }

    if num_changed > 0 {
        bail!("{} files changed", num_changed);
    }

    Ok(())
}
