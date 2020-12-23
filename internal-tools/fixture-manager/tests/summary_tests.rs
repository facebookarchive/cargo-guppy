// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{bail, Result};
use fixture_manager::{context::GenerateContext, summaries::SummaryContext, GenerateSummariesOpts};
use fixtures::json::JsonFixture;

/// Test that no checked in summaries have changed.
#[test]
fn summaries_unchanged() -> Result<()> {
    let mut num_changed = 0;

    for (name, fixture) in JsonFixture::all_fixtures() {
        let count = GenerateSummariesOpts::default_count();

        println!("generating {} summaries for {}...", count, name);

        let context: GenerateContext<SummaryContext> =
            GenerateContext::new(fixture, &count, false)?;

        for summary_pair in context {
            let item = summary_pair?;
            let is_changed = item.is_changed();
            if is_changed {
                num_changed += 1;
                println!("** {}:\n{}", item.path().display(), item.diff());
            }
        }
    }

    if num_changed > 0 {
        bail!("{} summaries changed", num_changed);
    }

    Ok(())
}
