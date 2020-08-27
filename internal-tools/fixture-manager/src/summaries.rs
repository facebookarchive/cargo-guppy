// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{bail, Result};
use fixtures::json::JsonFixture;
use guppy::graph::summaries::{diff::SummaryDiff, Summary};
use guppy_cmdlib::PackagesAndFeatures;
use once_cell::sync::Lazy;
use proptest_ext::ValueGenerator;
use std::path::{Path, PathBuf};
use std::{fs, io};

pub struct GenerateSummaryContext<'g> {
    fixture: &'g JsonFixture,
    skip_existing: bool,
    summary_template: PathBuf,
    iter_summaries: Box<dyn Iterator<Item = (usize, Summary)> + 'g>,
}

impl<'g> GenerateSummaryContext<'g> {
    pub fn new(fixture: &'g JsonFixture, count: usize, skip_existing: bool) -> Result<Self> {
        let summary_template = summary_template(fixture)?;
        let iter_summaries = Box::new(iter_summaries(fixture, count).enumerate());
        Ok(Self {
            fixture,
            skip_existing,
            summary_template,
            iter_summaries,
        })
    }
}

impl<'g> Iterator for GenerateSummaryContext<'g> {
    type Item = Result<SummaryPair>;

    fn next(&mut self) -> Option<Self::Item> {
        let (idx, summary) = self.iter_summaries.next()?;

        let mut summary_path = self.summary_template.clone();
        summary_path.set_file_name(format!("{}-{}.toml", self.fixture.name(), idx));
        let existing = if self.skip_existing {
            // In force mode, treat the on-disk summary as missing.
            None
        } else {
            match read_summary(&summary_path) {
                Ok(existing) => existing,
                Err(err) => return Some(Err(err)),
            }
        };

        Some(Ok(SummaryPair {
            idx,
            summary,
            existing,
            summary_path,
        }))
    }
}

#[derive(Clone, Debug)]
pub struct SummaryPair {
    pub idx: usize,
    pub summary: Summary,
    pub existing: Option<Summary>,
    pub summary_path: PathBuf,
}

impl SummaryPair {
    pub fn is_changed(&self) -> bool {
        match &self.existing {
            Some(existing) => {
                let diff = SummaryDiff::new(existing, &self.summary);
                diff.is_changed() || existing.metadata != self.summary.metadata
            }
            None => {
                // New summary: treat as changed.
                true
            }
        }
    }

    pub fn diff(&self) -> SummaryDiff {
        // Need to make this a static to allow lifetimes to work out.
        static EMPTY_SUMMARY: Lazy<Summary> = Lazy::new(Summary::default);

        SummaryDiff::new(
            self.existing.as_ref().unwrap_or_else(|| &*EMPTY_SUMMARY),
            &self.summary,
        )
    }

    pub fn write(&self, header: impl Into<String>) -> Result<()> {
        let mut out = header.into();
        if let Err(err) = self.summary.write_to_string(&mut out) {
            eprintln!("** Partially generated summary:\n{}", out);
            bail!(
                "Error while serializing TOML: {}\n\nPartially generated summary:\n{}",
                err,
                out
            );
        }

        Ok(fs::write(&self.summary_path, &out)?)
    }
}

fn iter_summaries<'g>(
    fixture: &'g JsonFixture,
    count: usize,
) -> impl Iterator<Item = Summary> + 'g {
    // Make a fresh generator for each summary so that filtering by --fixtures continues to
    // produce deterministic results.
    let mut generator = ValueGenerator::deterministic();

    let graph = fixture.graph();

    let packages_features_strategy = PackagesAndFeatures::strategy(graph);
    let cargo_opts_strategy = graph.prop010_cargo_options_strategy();

    (0..count).map(move |_| {
        let packages_features = generator.generate(&packages_features_strategy);
        let feature_query = packages_features
            .make_feature_query(graph)
            .expect("valid feature query");

        let cargo_opts = generator.generate(&cargo_opts_strategy);
        let cargo_set = feature_query
            .resolve_cargo(&cargo_opts)
            .expect("resolve_cargo succeeded");

        cargo_set
            .to_summary(&cargo_opts)
            .expect("generated summaries should serialize correctly")
    })
}

fn read_summary(summary_file: &Path) -> Result<Option<Summary>> {
    let data = match fs::read_to_string(summary_file) {
        Ok(data) => data,
        Err(err) => {
            if err.kind() == io::ErrorKind::NotFound {
                // Don't fail if the file wasn't found.
                return Ok(None);
            }
            return Err(err.into());
        }
    };

    Ok(Some(Summary::parse(&data)?))
}

/// Returns a summary template for this fixture, creating the directory as necessary.
///
/// The template can be used with `set_file_name` to change the file name.
fn summary_template(fixture: &JsonFixture) -> Result<PathBuf> {
    let mut summary_dir = fixture
        .abs_path()
        .parent()
        .expect("up to dirname of summary")
        .join("summaries");
    fs::create_dir_all(&summary_dir)?;

    summary_dir.push("REPLACE_THIS_FILE_NAME");
    Ok(summary_dir)
}
