// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Support for CLI operations with guppy, with structopt integration.
//!
//! This library allows translating command-line arguments into guppy's data structures.

#[cfg(feature = "proptest010")]
pub mod proptest;

use anyhow::Result;
use guppy::graph::feature::{
    all_filter, default_filter, feature_filter, none_filter, FeatureFilter, FeatureQuery,
};
use guppy::graph::PackageGraph;
use guppy::{MetadataCommand, Platform, TargetFeatures};
use std::env;
use std::path::PathBuf;
use structopt::StructOpt;

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
    /// Evaluates this struct against the given graph, and converts it into a `FeatureQuery`.
    pub fn make_feature_query<'g>(&self, graph: &'g PackageGraph) -> Result<FeatureQuery<'g>> {
        let package_query = if self.packages.is_empty() {
            graph.query_workspace()
        } else {
            graph.query_workspace_names(self.packages.iter().map(|s| s.as_str()))?
        };

        let base_filter: Box<dyn FeatureFilter> =
            match (self.all_features, self.no_default_features) {
                (true, _) => Box::new(all_filter()),
                (false, false) => Box::new(default_filter()),
                (false, true) => Box::new(none_filter()),
            };
        // TODO: support package/feature format
        // TODO: support feature name validation similar to cargo
        let feature_filter = feature_filter(base_filter, self.features.iter().map(|s| s.as_str()));

        Ok(graph
            .feature_graph()
            .query_packages(&package_query, feature_filter))
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
        Some(triple) => match Platform::new(triple, TargetFeatures::Unknown) {
            Some(platform) => Ok(Some(platform)),
            None => anyhow::bail!("unrecognized triple '{}'", triple),
        },
        None => Ok(default_fn()),
    }
}
