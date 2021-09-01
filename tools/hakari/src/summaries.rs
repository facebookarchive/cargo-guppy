// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Generate summaries from `HakariBuilder` instances.
//!
//! Requires the `summaries` feature to be enabled.

use crate::{HakariBuilder, TomlOutError, UnifyTargetHost};
use guppy::{
    graph::{
        cargo::CargoResolverVersion,
        summaries::{PlatformSummary, SummaryId},
        PackageGraph,
    },
    TargetSpecError,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt};
use toml::Serializer;

/// A `HakariBuilder` in serializable form.
///
/// Requires the `summaries` feature to be enabled.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct HakariBuilderSummary {
    /// The name of the Hakari package in the workspace.
    pub hakari_package: Option<String>,

    /// The resolver version used.
    pub version: CargoResolverVersion,

    /// Whether the builder was run in verify mode.
    #[serde(default)]
    pub verify_mode: bool,

    /// Unification across target and host.
    #[serde(default)]
    pub unify_target_host: UnifyTargetHost,

    /// Whether all dependencies were unified.
    #[serde(default)]
    pub unify_all: bool,

    /// The platforms used by the `HakariBuilder`.
    pub platforms: Vec<PlatformSummary>,

    /// The list of omitted packages.
    pub omitted_packages: BTreeSet<SummaryId>,
}

impl HakariBuilderSummary {
    /// Creates a new `HakariBuilderSummary` from a builder.
    ///
    /// Requires the `summaries` feature to be enabled.
    ///
    /// Returns an error if there are any custom platforms. Serializing custom platforms is
    /// currently unsupported.
    pub fn new(builder: &HakariBuilder<'_>) -> Result<Self, TargetSpecError> {
        Ok(Self {
            hakari_package: builder
                .hakari_package()
                .map(|package| package.name().to_string()),
            platforms: builder
                .platforms()
                .iter()
                .map(|platform| PlatformSummary::new(platform))
                .collect::<Result<Vec<_>, _>>()?,
            version: builder.resolver_version(),
            verify_mode: builder.verify_mode(),
            omitted_packages: builder
                .omitted_packages_only()
                .map(|package_id| {
                    builder
                        .graph()
                        .metadata(package_id)
                        .expect("valid package ID")
                        .to_summary_id()
                })
                .collect(),
            unify_target_host: builder.unify_target_host(),
            unify_all: builder.unify_all(),
        })
    }

    /// Creates a `HakariBuilder` from this summary and a `PackageGraph`.
    ///
    /// Returns an error if this summary references a package that's not present, or if there was
    /// some other issue while creating a `HakariBuilder` from this summary.
    pub fn to_hakari_builder<'g>(
        &self,
        graph: &'g PackageGraph,
    ) -> Result<HakariBuilder<'g>, guppy::Error> {
        HakariBuilder::from_summary(graph, self)
    }

    /// Serializes this summary to a TOML string.
    ///
    /// Returns an error if writing out the TOML was unsuccessful.
    pub fn to_string(&self) -> Result<String, toml::ser::Error> {
        let mut dst = String::new();
        self.write_to_string(&mut dst)?;
        Ok(dst)
    }

    /// Serializes this summary to a TOML string, and adds `#` comment markers to the beginning of
    /// each line.
    ///
    /// Returns an error if writing out the TOML was unsuccessful.
    pub fn write_comment(&self, mut out: impl fmt::Write) -> Result<(), TomlOutError> {
        // Begin with a comment.
        let summary = self.to_string().map_err(|err| TomlOutError::Toml {
            context: "while serializing HakariBuilderSummary as comment".into(),
            err,
        })?;
        for line in summary.lines() {
            if line.is_empty() {
                writeln!(out, "#")?;
            } else {
                writeln!(out, "# {}", line)?;
            }
        }
        Ok(())
    }

    /// Writes out the contents of this summary as TOML to the given string.
    ///
    /// Returns an error if writing out the TOML was unsuccessful.
    pub fn write_to_string(&self, dst: &mut String) -> Result<(), toml::ser::Error> {
        let mut serializer = Serializer::pretty(dst);
        serializer.pretty_array(false);
        self.serialize(&mut serializer)
    }
}

impl<'g> HakariBuilder<'g> {
    /// Converts this `HakariBuilder` to a serializable summary.
    ///
    /// Requires the `summaries` feature to be enabled.
    ///
    /// Returns an error if there are any custom platforms. Serializing custom platforms is
    /// currently unsupported.
    pub fn to_summary(&self) -> Result<HakariBuilderSummary, TargetSpecError> {
        HakariBuilderSummary::new(self)
    }
}
