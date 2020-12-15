// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::context::ContextImpl;
use anyhow::Result;
use fixtures::json::JsonFixture;
use hakari::{diffy::PatchFormatter, Hakari, HakariBuilder, HakariCargoToml, TomlOptions};
use once_cell::sync::Lazy;
use proptest_ext::ValueGenerator;
use std::path::{Path, PathBuf};

pub struct HakariTomlContext;

impl<'g> ContextImpl<'g> for HakariTomlContext {
    type IterArgs = usize;
    type IterItem = (usize, HakariTomlItem<'g>);
    type Existing = HakariCargoToml;

    fn dir_name(fixture: &'g JsonFixture) -> PathBuf {
        fixture
            .abs_path()
            .parent()
            .expect("up to dirname of summary")
            .join("hakari")
    }

    fn file_name(fixture: &'g JsonFixture, &(count, _): &Self::IterItem) -> String {
        format!("{}-{}.toml", fixture.name(), count)
    }

    fn iter(
        fixture: &'g JsonFixture,
        &count: &Self::IterArgs,
    ) -> Box<dyn Iterator<Item = Self::IterItem> + 'g> {
        // Make a fresh generator for each output so that filtering by --fixtures continues to
        // produce deterministic results.
        let mut generator = ValueGenerator::deterministic();

        let graph = fixture.graph();
        // TODO: add tests for hakari id -- none of our fixtures appear to have a
        // workspace-hack or other Hakari package
        let hakari_builder_strategy = HakariBuilder::prop010_strategy(graph, None);

        let iter = (0..count).map(move |idx| {
            // The partial clones mean that a change to the algorithm in part of the strategy won't
            // affect the rest of it.
            let mut iter_generator = generator.partial_clone();
            let builder = iter_generator
                .partial_clone()
                .generate(&hakari_builder_strategy);
            let hakari = builder.compute();
            let mut options = TomlOptions::new();
            options.set_builder_summary(true).set_absolute_paths(true);
            let toml = hakari
                .to_toml_string(&options)
                .expect("to_toml_string worked");

            (idx, HakariTomlItem { hakari, toml })
        });
        Box::new(iter)
    }

    fn parse_existing(path: &Path, contents: String) -> Result<Self::Existing> {
        Ok(HakariCargoToml::new_in_memory(path, contents)?)
    }

    fn is_changed((_, item): &Self::IterItem, existing: &Self::Existing) -> bool {
        existing.is_changed(&item.toml)
    }

    fn diff(
        _fixture: &'g JsonFixture,
        (_, item): &Self::IterItem,
        existing: Option<&Self::Existing>,
    ) -> String {
        static DEFAULT_EXISTING: Lazy<HakariCargoToml> = Lazy::new(|| {
            let contents = format!(
                "{}{}",
                HakariCargoToml::BEGIN_SECTION,
                HakariCargoToml::END_SECTION
            );
            HakariCargoToml::new_in_memory("default", contents)
                .expect("contents are in correct format")
        });

        let existing = existing.unwrap_or_else(|| &*DEFAULT_EXISTING);

        let diff = existing.diff_toml(&item.toml);
        let formatter = PatchFormatter::new();
        format!("{}", formatter.fmt_patch(&diff))
    }

    fn write_to_string(
        fixture: &'g JsonFixture,
        (_, item): &Self::IterItem,
        out: &mut String,
    ) -> Result<()> {
        // XXX this should be unified with `DEFAULT_EXISTING` somehow, bleh
        let out_contents = format!(
            "# This file is @generated. To regenerate, run:\n\
             #    cargo run -p fixture-manager -- generate-hakari --fixture {}\n\
             \n\
             ### BEGIN HAKARI SECTION\n\
             \n\
             ### END HAKARI SECTION\n\
             \n\
             # This part of the file should be preserved at the end.\n",
            fixture.name()
        );

        let new_toml = HakariCargoToml::new_in_memory("bogus", out_contents)?;
        Ok(new_toml.write_to_fmt(&item.toml, out)?)
    }
}

pub struct HakariTomlItem<'g> {
    #[allow(dead_code)]
    hakari: Hakari<'g, 'static>,
    toml: String,
}
