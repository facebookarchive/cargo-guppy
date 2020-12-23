// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::context::ContextImpl;
use anyhow::Result;
use fixtures::json::JsonFixture;
use guppy::graph::summaries::{diff::SummaryDiff, Summary};
use guppy_cmdlib::PackagesAndFeatures;
use once_cell::sync::Lazy;
use proptest_ext::ValueGenerator;
use std::{
    fmt::Write,
    path::{Path, PathBuf},
};

pub struct SummaryContext;

impl<'g> ContextImpl<'g> for SummaryContext {
    type IterArgs = usize;
    type IterItem = (usize, Summary);
    type Existing = Summary;

    fn dir_name(fixture: &'g JsonFixture) -> PathBuf {
        fixture
            .abs_path()
            .parent()
            .expect("up to dirname of summary")
            .join("summaries")
    }

    fn file_name(fixture: &'g JsonFixture, &(count, _): &Self::IterItem) -> String {
        format!("{}-{}.toml", fixture.name(), count)
    }

    fn iter(
        fixture: &'g JsonFixture,
        &count: &Self::IterArgs,
    ) -> Box<dyn Iterator<Item = Self::IterItem> + 'g> {
        // Make a fresh generator for each summary so that filtering by --fixtures continues to
        // produce deterministic results.
        let mut generator = ValueGenerator::deterministic();

        let graph = fixture.graph();

        let packages_features_strategy = PackagesAndFeatures::strategy(graph);
        let cargo_opts_strategy = graph.prop010_cargo_options_strategy();

        let iter = (0..count).map(move |idx| {
            let packages_features = generator.generate(&packages_features_strategy);
            let feature_query = packages_features
                .make_feature_query(graph)
                .expect("valid feature query");

            let cargo_opts = generator.generate(&cargo_opts_strategy);
            let cargo_set = feature_query
                .resolve_cargo(&cargo_opts)
                .expect("resolve_cargo succeeded");

            (
                idx,
                cargo_set
                    .to_summary(&cargo_opts)
                    .expect("generated summaries should serialize correctly"),
            )
        });

        Box::new(iter)
    }

    fn parse_existing(_: &Path, contents: String) -> Result<Self::Existing> {
        Ok(Summary::parse(&contents)?)
    }

    fn is_changed((_, summary): &Self::IterItem, existing: &Self::Existing) -> bool {
        let diff = SummaryDiff::new(existing, &summary);
        diff.is_changed() || existing.metadata != summary.metadata
    }

    fn diff((_, summary): &Self::IterItem, existing: Option<&Self::Existing>) -> String {
        // Need to make this a static to allow lifetimes to work out.
        static EMPTY_SUMMARY: Lazy<Summary> = Lazy::new(Summary::default);

        let existing = match existing {
            Some(summary) => summary,
            None => &*EMPTY_SUMMARY,
        };

        let diff = SummaryDiff::new(existing, &summary);
        format!("{}", diff.report())
    }

    fn write_to_string(
        fixture: &'g JsonFixture,
        (_, summary): &Self::IterItem,
        out: &mut String,
    ) -> Result<()> {
        writeln!(
            out,
            "# This summary was @generated. To regenerate, run:\n\
             #   cargo run -p fixture-manager -- generate-summaries --fixture {}\n",
            fixture.name()
        )?;

        summary.write_to_string(out)?;
        Ok(())
    }
}
