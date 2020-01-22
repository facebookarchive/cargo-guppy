// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Contains types that describe errors and warnings that `guppy` methods can return.

use crate::PackageId;
use cargo_metadata::Error as MetadataError;
use serde_json;
use std::error;
use std::fmt;
use std::io;

use Error::*;

/// Error type describing the sorts of errors `guppy` can return.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    // The config API isn't public yet, so hide errors within it.
    #[doc(hidden)]
    ConfigIoError(io::Error),
    #[doc(hidden)]
    ConfigParseError(toml::de::Error),
    /// An error occurred while executing `cargo metadata`.
    CommandError(MetadataError),
    /// An error occurred while parsing cargo metadata JSON.
    MetadataParseError(serde_json::Error),
    /// An error occurred while constructing a `PackageGraph` from parsed metadata.
    PackageGraphConstructError(String),
    /// A package ID was unknown to this `PackageGraph`.
    UnknownPackageId(PackageId),
    /// An internal error occurred within this `PackageGraph`.
    PackageGraphInternalError(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigIoError(err) => write!(f, "Error while reading config file: {}", err),
            ConfigParseError(err) => write!(f, "Error while parsing config file: {}", err),
            CommandError(err) => write!(f, "Error while executing 'cargo metadata': {}", err),
            MetadataParseError(err) => write!(
                f,
                "Error while parsing 'cargo metadata' JSON output: {}",
                err
            ),
            PackageGraphConstructError(msg) => {
                write!(f, "Error while computing package graph: {}", msg)
            }
            UnknownPackageId(id) => write!(f, "Unknown package ID: {}", id),
            PackageGraphInternalError(msg) => write!(f, "Internal error in package graph: {}", msg),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            ConfigIoError(err) => Some(err),
            ConfigParseError(err) => Some(err),
            MetadataParseError(err) => Some(err),
            CommandError(_) => None,
            PackageGraphConstructError(_) => None,
            UnknownPackageId(_) => None,
            PackageGraphInternalError(_) => None,
        }
    }
}

/// Describes warnings emitted during feature graph construction.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum FeatureGraphWarning {
    /// A feature that was requested is missing from a package.
    MissingFeature {
        /// The stage of building the feature graph where the warning occurred.
        stage: FeatureBuildStage,
        /// The package ID for which the feature was requested.
        package_id: PackageId,
        /// The name of the feature.
        feature_name: String,
    },
}

impl fmt::Display for FeatureGraphWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use FeatureGraphWarning::*;
        match self {
            MissingFeature {
                stage,
                package_id,
                feature_name,
            } => write!(
                f,
                "{}: for package '{}', missing feature '{}'",
                stage, package_id, feature_name
            ),
        }
    }
}

/// Describes the stage of construction at which a feature graph warning occurred.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum FeatureBuildStage {
    /// The warning occurred while adding edges for the `[features]` section of `Cargo.toml`.
    AddNamedFeatureEdges {
        /// The package ID for which edges were being added.
        package_id: PackageId,
        /// The feature name from which edges were being added.
        from_feature: String,
    },
    /// The warning occurred while adding dependency edges.
    AddDependencyEdges {
        /// The package ID for which edges were being added.
        package_id: PackageId,
        /// The name of the dependency.
        dep_name: String,
    },
}

impl fmt::Display for FeatureBuildStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use FeatureBuildStage::*;
        match self {
            AddNamedFeatureEdges {
                package_id,
                from_feature,
            } => write!(
                f,
                "for package '{}', while adding named feature edges from '{}'",
                package_id, from_feature
            ),
            AddDependencyEdges {
                package_id,
                dep_name,
            } => write!(
                f,
                "for package '{}', while adding edges for dependency '{}'",
                package_id, dep_name,
            ),
        }
    }
}
