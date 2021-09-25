// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Manage configuration and generate summaries for `hakari`.
//!
//! Requires the `summaries` feature to be enabled.

use crate::{HakariBuilder, HakariOutputOptions, TomlOutError, UnifyTargetHost};
use guppy::{
    graph::{cargo::CargoResolverVersion, summaries::PackageSetSummary, PackageGraph},
    TargetSpecError,
};
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use toml::Serializer;

/// Configuration for `hakari`.
///
/// Requires the `summaries` feature to be enabled.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub struct HakariConfig {
    /// Builder options.
    #[serde(flatten)]
    pub builder: HakariBuilderSummary,

    /// Output options.
    #[serde(default)]
    pub output: OutputOptionsSummary,
}

impl FromStr for HakariConfig {
    type Err = toml::de::Error;

    /// Deserializes a [`HakariConfig`] from the given TOML string.
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        toml::from_str(input)
    }
}

/// A `HakariBuilder` in serializable form. This forms the configuration file format for `hakari`.
///
/// For an example, see the
/// [cargo-hakari README](https://github.com/facebookincubator/cargo-guppy/tree/main/tools/hakari#configuration).
///
/// Requires the `summaries` feature to be enabled.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub struct HakariBuilderSummary {
    /// The name of the Hakari package in the workspace.
    pub hakari_package: Option<String>,

    /// The Cargo resolver version used.
    ///
    /// For more information, see the documentation for [`CargoResolverVersion`].
    #[serde(alias = "version")]
    pub resolver: CargoResolverVersion,

    /// Unification across target and host.
    #[serde(default)]
    pub unify_target_host: UnifyTargetHost,

    /// Whether all dependencies were unified.
    #[serde(default)]
    pub unify_all: bool,

    /// The platforms used by the `HakariBuilder`.
    #[serde(default)]
    pub platforms: Vec<String>,

    /// The list of omitted packages.
    #[serde(default)]
    pub omitted_packages: PackageSetSummary,
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
                .map(|triple_str| triple_str.to_owned())
                .collect::<Vec<_>>(),
            resolver: builder.resolver(),
            omitted_packages: PackageSetSummary::from_package_ids(
                builder.graph(),
                builder.omitted_packages_only(),
            )
            .expect("all package IDs are valid"),
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

/// Options for `hakari` TOML output, in serializable form.
///
/// TODO: add a configuration.md file.
#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub struct OutputOptionsSummary {
    /// Output exact versions in package version fields.
    #[serde(default)]
    exact_versions: bool,

    /// Output absolute paths for path dependencies.
    #[serde(default)]
    absolute_paths: bool,

    /// Output a [`HakariBuilderSummary`] as comments.
    #[serde(default)]
    builder_summary: bool,
}

impl OutputOptionsSummary {
    /// Creates a new `OutputOptionsSummary`.
    pub fn new(options: &HakariOutputOptions) -> Self {
        Self {
            exact_versions: options.exact_versions,
            absolute_paths: options.absolute_paths,
            builder_summary: options.builder_summary,
        }
    }

    /// Converts this summary to the options.
    pub fn to_options(&self) -> HakariOutputOptions {
        HakariOutputOptions {
            exact_versions: self.exact_versions,
            absolute_paths: self.absolute_paths,
            builder_summary: self.builder_summary,
        }
    }
}
