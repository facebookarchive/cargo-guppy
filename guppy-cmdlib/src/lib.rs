// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Support for CLI operations with guppy, with structopt integration.
//!
//! This library allows translating command-line arguments into guppy's data structures.

#[cfg(feature = "proptest1")]
pub mod proptest;

use anyhow::Result;
use guppy::{
    graph::{
        cargo::{CargoResolverVersion, InitialsPlatform},
        feature::{feature_filter, FeatureSet, StandardFeatures},
        PackageGraph,
    },
    platform::{Platform, PlatformSpec, TargetFeatures},
    MetadataCommand,
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

    #[structopt(long = "features-only", number_of_values = 1)]
    /// Packages that take part in feature unification but aren't in the result set (default: none)
    pub features_only: Vec<String>,

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
    /// Evaluates this struct against the given graph, and converts it into the initials and
    /// features-only `FeatureSet`s.
    pub fn make_feature_sets<'g>(
        &self,
        graph: &'g PackageGraph,
    ) -> Result<(FeatureSet<'g>, FeatureSet<'g>)> {
        let package_set = if self.packages.is_empty() {
            graph.resolve_workspace()
        } else {
            graph.resolve_workspace_names(self.packages.iter())?
        };
        let features_only_set = if self.features_only.is_empty() {
            graph.resolve_none()
        } else {
            graph.resolve_workspace_names(self.features_only.iter())?
        };

        let base_filter = match (self.all_features, self.no_default_features) {
            (true, _) => StandardFeatures::All,
            (false, false) => StandardFeatures::Default,
            (false, true) => StandardFeatures::None,
        };
        // TODO: support package/feature format
        // TODO: support feature name validation similar to cargo
        let mut feature_filter =
            feature_filter(base_filter, self.features.iter().map(|s| s.as_str()));

        Ok((
            package_set.to_feature_set(&mut feature_filter),
            features_only_set.to_feature_set(&mut feature_filter),
        ))
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

arg_enum! {
    enum InitialsPlatformCmd {
        Host,
        Standard,
        ProcMacrosOnTarget,
    }
}

/// Support for options like the Cargo resolver version.
#[derive(Clone, Debug, StructOpt)]
pub struct CargoResolverOpts {
    #[structopt(long = "include-dev")]
    /// Include dev-dependencies of initial packages (default: false)
    pub include_dev: bool,

    #[structopt(long = "initials-platform", parse(try_from_str = parse_initials_platform))]
    #[structopt(possible_values = &InitialsPlatformCmd::variants(), case_insensitive = true, default_value = "Standard")]
    /// Include initial proc-macros on target platform (default: false)
    pub initials_platform: InitialsPlatform,

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

/// Parses a named initials platform into an InitialsPlatform.
pub fn parse_initials_platform(s: &str) -> Result<InitialsPlatform, String> {
    let p = s.parse::<InitialsPlatformCmd>()?;
    match p {
        InitialsPlatformCmd::Host => Ok(InitialsPlatform::Host),
        InitialsPlatformCmd::Standard => Ok(InitialsPlatform::Standard),
        InitialsPlatformCmd::ProcMacrosOnTarget => Ok(InitialsPlatform::ProcMacrosOnTarget),
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
pub fn string_to_platform_spec(s: Option<&str>) -> Result<PlatformSpec> {
    match s {
        Some("current") => Ok(PlatformSpec::current()?),
        Some("always") => Ok(PlatformSpec::Always),
        Some("any") => Ok(PlatformSpec::Any),
        Some(triple) => Ok(Platform::new(triple.to_owned(), TargetFeatures::Unknown)?.into()),
        None => Ok(PlatformSpec::Any),
    }
}
