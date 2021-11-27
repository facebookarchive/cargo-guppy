// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Manage configuration and generate summaries for `hakari`.
//!
//! Requires the `cli-support` feature to be enabled.

use crate::{
    hakari::DepFormatVersion, HakariBuilder, HakariOutputOptions, TomlOutError, UnifyTargetHost,
};
use guppy::{
    errors::TargetSpecError,
    graph::{cargo::CargoResolverVersion, summaries::PackageSetSummary, PackageGraph},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt, str::FromStr};
use toml::Serializer;

/// The location of the configuration used by `cargo hakari`, relative to the workspace root.
pub static DEFAULT_CONFIG_PATH: &str = ".config/hakari.toml";

/// The fallback location, used by previous versions of `cargo hakari`.
pub static FALLBACK_CONFIG_PATH: &str = ".guppy/hakari.toml";

/// Configuration for `hakari`.
///
/// Requires the `cli-support` feature to be enabled.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub struct HakariConfig {
    /// Builder options.
    #[serde(flatten)]
    pub builder: HakariBuilderSummary,

    /// Output options.
    #[serde(flatten)]
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
/// Requires the `cli-support` feature to be enabled.
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
    pub output_single_feature: bool,

    /// Version of `workspace-hack = ...` lines in other `Cargo.toml` to use.
    #[serde(default)]
    pub dep_format_version: DepFormatVersion,

    /// The platforms used by the `HakariBuilder`.
    #[serde(default)]
    pub platforms: Vec<String>,

    /// The list of packages excluded during graph traversals.
    #[serde(default)]
    pub traversal_excludes: PackageSetSummary,

    /// The list of packages excluded from the final output.
    #[serde(default)]
    pub final_excludes: PackageSetSummary,

    /// The list of alternate registries, as a map of name to URL.
    ///
    /// This is a temporary workaround until [Cargo issue #9052](https://github.com/rust-lang/cargo/issues/9052)
    /// is resolved.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        with = "registries_impl"
    )]
    pub registries: BTreeMap<String, String>,
}

impl HakariBuilderSummary {
    /// Creates a new `HakariBuilderSummary` from a builder.
    ///
    /// Requires the `cli-support` feature to be enabled.
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
            traversal_excludes: PackageSetSummary::from_package_ids(
                builder.graph(),
                builder.traversal_excludes_only(),
            )
            .expect("all package IDs are valid"),
            final_excludes: PackageSetSummary::from_package_ids(
                builder.graph(),
                builder.final_excludes(),
            )
            .expect("all package IDs are valid"),
            registries: builder
                .registries
                .iter()
                .map(|(name, url)| (name.clone(), url.clone()))
                .collect(),
            unify_target_host: builder.unify_target_host(),
            output_single_feature: builder.output_single_feature(),
            dep_format_version: builder.dep_format_version,
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
    /// Requires the `cli-support` feature to be enabled.
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

mod registries_impl {
    use super::*;
    use serde::{Deserializer, Serializer};

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RegistryDe {
        index: String,
    }

    #[derive(Debug, Serialize)]
    struct RegistrySer<'a> {
        index: &'a str,
    }

    /// Serializes a path using forward slashes.
    pub fn serialize<S>(
        registry_map: &BTreeMap<String, String>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let ser_map: BTreeMap<_, _> = registry_map
            .iter()
            .map(|(name, index)| {
                (
                    name.as_str(),
                    RegistrySer {
                        index: index.as_str(),
                    },
                )
            })
            .collect();
        ser_map.serialize(serializer)
    }

    /// Deserializes a path, converting forward slashes to backslashes.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<BTreeMap<String, String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let de_map = BTreeMap::<String, RegistryDe>::deserialize(deserializer)?;
        let registry_map = de_map
            .into_iter()
            .map(|(name, RegistryDe { index })| (name, index))
            .collect();
        Ok(registry_map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixtures::json::*;

    #[test]
    fn parse_registries() {
        static PARSE_REGISTRIES_INPUT: &str = r#"
        resolver = "2"

        [traversal-excludes]
        third-party = [
            { name = "serde_derive", registry = "my-registry" },
        ]

        [registries]
        my-registry = { index = "https://github.com/fakeorg/crates.io-index" }
        your-registry = { index = "https://foobar" }
        "#;

        let summary: HakariBuilderSummary =
            toml::from_str(PARSE_REGISTRIES_INPUT).expect("failed to parse toml");
        // Need an arbitrary graph for this.
        let builder = summary
            .to_hakari_builder(JsonFixture::metadata_alternate_registries().graph())
            .expect("summary => builder conversion");

        assert_eq!(
            summary.registries.get("my-registry").map(|s| s.as_str()),
            Some(METADATA_ALTERNATE_REGISTRY_URL),
            "my-registry is correct"
        );
        assert_eq!(
            summary.registries.get("your-registry").map(|s| s.as_str()),
            Some("https://foobar"),
            "your-registry is correct"
        );

        let summary2 = builder.to_summary().expect("builder => summary conversion");
        let builder2 = summary
            .to_hakari_builder(JsonFixture::metadata_alternate_registries().graph())
            .expect("summary2 => builder2 conversion");
        assert_eq!(
            builder.traversal_excludes, builder2.traversal_excludes,
            "builder == builder2 traversal excludes"
        );

        let serialized = toml::to_string(&summary2).expect("serialized to TOML correctly");
        let summary3: HakariBuilderSummary =
            toml::from_str(&serialized).expect("deserialized from TOML correctly");
        assert_eq!(
            summary2, summary3,
            "summary => serialized => summary roundtrip"
        );
    }
}
