// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Support for CLI operations with guppy, with structopt integration.
//!
//! This library allows translating command-line arguments into guppy's data structures.

#[cfg(feature = "proptest010")]
pub mod proptest;

use anyhow::Result;
use guppy::{
    graph::{
        cargo::CargoResolverVersion,
        feature::{feature_filter, FeatureSet, StandardFeatures},
        PackageGraph,
    },
    MetadataCommand, Platform, TargetFeatures,
};
use std::{env, path::PathBuf};
use structopt::{clap::arg_enum, StructOpt};

/// Support for packages and features.
///
/// The options here mirror Cargo's.
#[derive(Debug, StructOpt)]
pub struct PackagesAndFeatures {
    #[structopt(long = "package", short = "p", number_of_values = 1)]
    /// Packages to start the query from (default: entire workspace)
    pub packages: Vec<String>,

    // TODO: support --workspace and --exclude
    /// List of features to activate across all packages
    #[structopt(long = "features", use_delimiter = true)]
    pub features: Vec<String>,

    /// Activate all available features
    #[structopt(long = "all-features")]
    pub all_features: bool,

    /// Do not activate the `default` feature
    #[structopt(long = "no-default-features")]
    pub no_default_features: bool,
}

impl PackagesAndFeatures {
    /// Evaluates this struct against the given graph, and converts it into a `FeatureSet`.
    pub fn make_feature_set<'g>(&self, graph: &'g PackageGraph) -> Result<FeatureSet<'g>> {
        let package_set = if self.packages.is_empty() {
            graph.resolve_workspace()
        } else {
            graph.resolve_workspace_names(self.packages.iter().map(|s| s.as_str()))?
        };

        let base_filter = match (self.all_features, self.no_default_features) {
            (true, _) => StandardFeatures::All,
            (false, false) => StandardFeatures::Default,
            (false, true) => StandardFeatures::None,
        };
        // TODO: support package/feature format
        // TODO: support feature name validation similar to cargo
        let feature_filter = feature_filter(base_filter, self.features.iter().map(|s| s.as_str()));

        Ok(package_set.to_feature_set(feature_filter))
    }
}

arg_enum! {
    // Identical to guppy's CargoResolverVersion, except with additional string metadata generated
    // for matching.
    enum ResolverVersion {
        V1,
        V1Install,
        V2,
    }
}

/// Support for options like the Cargo resolver version.
#[derive(Clone, Debug, StructOpt)]
pub struct CargoResolverOpts {
    #[structopt(long = "include-dev")]
    /// Include dev-dependencies of initial packages (default: false)
    pub include_dev: bool,

    #[structopt(long = "proc-macros-on-target")]
    /// Include initial proc-macros on target platform (default: false)
    pub proc_macros_on_target: bool,

    #[structopt(long = "resolver-version", parse(try_from_str = parse_resolver_version))]
    #[structopt(possible_values = &ResolverVersion::variants(), case_insensitive = true, default_value = "V1")]
    pub resolver_version: CargoResolverVersion,
}

/// Parses a named resolver version into a CargoResolverVersion.
pub fn parse_resolver_version(s: &str) -> Result<CargoResolverVersion, String> {
    let version = s.parse::<ResolverVersion>()?;
    match version {
        ResolverVersion::V1 => Ok(CargoResolverVersion::V1),
        ResolverVersion::V1Install => Ok(CargoResolverVersion::V1Install),
        ResolverVersion::V2 => Ok(CargoResolverVersion::V2),
    }
}

/// Context for invoking the `cargo metadata` command.
///
/// The options mirror Cargo's.
#[derive(Clone, Debug, StructOpt)]
pub struct CargoMetadataOptions {
    /// Path to Cargo.toml
    #[structopt(long = "manifest-path")]
    pub manifest_path: Option<PathBuf>,
}

impl CargoMetadataOptions {
    /// Returns the current directory.
    pub fn current_dir(&self) -> Result<PathBuf> {
        Ok(env::current_dir()?)
    }

    /// Returns the absolute canonical manifest path.
    pub fn abs_manifest_path(&self) -> Result<PathBuf> {
        let cwd = self.current_dir()?;
        let path = match &self.manifest_path {
            Some(path) => cwd.join(path),
            None => cwd.join("Cargo.toml"),
        };
        Ok(path.canonicalize()?)
    }

    /// Evaluates this struct and creates a `MetadataCommand`.
    pub fn make_command(&self) -> MetadataCommand {
        let mut command = MetadataCommand::new();
        if let Some(manifest_path) = &self.manifest_path {
            command.manifest_path(manifest_path);
        }
        command
    }
}

/// Parse a given triple, the string "current", or "any", into a platform.
///
/// TODO: This should eventually support JSON specs as well, probably.
pub fn triple_to_platform<'a>(
    triple: Option<&str>,
    default_fn: impl FnOnce() -> Option<Platform<'a>>,
) -> Result<Option<Platform<'a>>> {
    match triple {
        Some("current") => Ok(Platform::current()),
        Some("any") => Ok(None),
        Some(triple) => Ok(Some(Platform::new(triple, TargetFeatures::Unknown)?)),
        None => Ok(default_fn()),
    }
}
