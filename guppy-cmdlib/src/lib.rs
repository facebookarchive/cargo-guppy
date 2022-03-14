// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Support for CLI operations with guppy, with structopt integration.
//!
//! This library allows translating command-line arguments into guppy's data structures.

#[cfg(feature = "proptest1")]
pub mod proptest;

use clap::{ArgEnum, Parser};
use color_eyre::eyre::Result;
use guppy::{
    graph::{
        cargo::{CargoResolverVersion, InitialsPlatform},
        feature::{named_feature_filter, FeatureSet, StandardFeatures},
        PackageGraph,
    },
    platform::{Platform, PlatformSpec, TargetFeatures},
    MetadataCommand,
};
use std::{env, path::PathBuf};

/// Support for packages and features.
///
/// The options here mirror Cargo's.
#[derive(Debug, Parser)]
pub struct PackagesAndFeatures {
    #[clap(long = "package", short = 'p')]
    /// Packages to start the query from (default: entire workspace)
    pub packages: Vec<String>,

    #[clap(long = "features-only")]
    /// Packages that take part in feature unification but aren't in the result set (default: none)
    pub features_only: Vec<String>,

    // TODO: support --workspace and --exclude
    /// List of features to activate across all packages
    #[clap(long = "features", use_value_delimiter = true)]
    pub features: Vec<String>,

    /// Activate all available features
    #[clap(long = "all-features")]
    pub all_features: bool,

    /// Do not activate the `default` feature
    #[clap(long = "no-default-features")]
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
            named_feature_filter(base_filter, self.features.iter().map(|s| s.as_str()));

        Ok((
            package_set.to_feature_set(&mut feature_filter),
            features_only_set.to_feature_set(&mut feature_filter),
        ))
    }
}

// Identical to guppy's CargoResolverVersion, except with additional string metadata generated
// for matching.
#[derive(ArgEnum, Clone, Copy, Debug)]
pub enum CargoResolverVersionCmd {
    V1,
    V1Install,
    V2,
}

#[derive(ArgEnum, Clone, Copy, Debug)]
pub enum InitialsPlatformCmd {
    Host,
    Standard,
    ProcMacrosOnTarget,
}

/// Support for options like the Cargo resolver version.
#[derive(Clone, Debug, Parser)]
pub struct CargoResolverOpts {
    #[clap(long = "include-dev")]
    /// Include dev-dependencies of initial packages (default: false)
    pub include_dev: bool,

    #[clap(long = "initials-platform")]
    #[clap(arg_enum, default_value_t = InitialsPlatformCmd::Standard)]
    /// Include initial proc-macros on target platform (default: false)
    pub initials_platform: InitialsPlatformCmd,

    #[clap(long = "resolver-version")]
    #[clap(arg_enum, default_value_t = CargoResolverVersionCmd::V1)]
    /// Cargo resolver version to use
    pub resolver_version: CargoResolverVersionCmd,
}

impl CargoResolverVersionCmd {
    /// Converts to guppy's CargoResolverVersion.
    pub fn to_guppy(self) -> CargoResolverVersion {
        match self {
            CargoResolverVersionCmd::V1 => CargoResolverVersion::V1,
            CargoResolverVersionCmd::V1Install => CargoResolverVersion::V1Install,
            CargoResolverVersionCmd::V2 => CargoResolverVersion::V2,
        }
    }
}

impl InitialsPlatformCmd {
    /// Converts to guppy's InitialsPlatform.
    pub fn to_guppy(self) -> InitialsPlatform {
        match self {
            InitialsPlatformCmd::Host => InitialsPlatform::Host,
            InitialsPlatformCmd::Standard => InitialsPlatform::Standard,
            InitialsPlatformCmd::ProcMacrosOnTarget => InitialsPlatform::ProcMacrosOnTarget,
        }
    }
}

/// Context for invoking the `cargo metadata` command.
///
/// The options mirror Cargo's.
#[derive(Clone, Debug, Parser)]
pub struct CargoMetadataOptions {
    /// Path to Cargo.toml
    #[clap(long)]
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
