// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Contains types that describe errors and warnings that `guppy` methods can return.

use crate::PackageId;
use std::error;
use std::fmt;

use crate::graph::feature::FeatureId;
use std::path::PathBuf;
use Error::*;

/// Error type describing the sorts of errors `guppy` can return.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// An error occurred while executing `cargo metadata`.
    CommandError(Box<dyn error::Error + Send + Sync>),
    /// An error occurred while parsing `cargo metadata` JSON.
    MetadataParseError(serde_json::Error),
    /// An error occurred while serializing `cargo metadata` JSON.
    MetadataSerializeError(serde_json::Error),
    /// An error occurred while constructing a `PackageGraph` from parsed metadata.
    PackageGraphConstructError(String),
    /// A package ID was unknown to this `PackageGraph`.
    UnknownPackageId(PackageId),
    /// A feature ID was unknown to this `FeatureGraph`.
    UnknownFeatureId(PackageId, Option<String>),
    /// A package specified by path was unknown to this workspac.e
    UnknownWorkspacePath(PathBuf),
    /// A package specified by name was unknown to this workspace.
    UnknownWorkspaceName(String),
    /// An error occured while computing a `CargoSet`.
    CargoSetError(String),
    /// An internal error occurred within this `PackageGraph`.
    PackageGraphInternalError(String),
    /// An internal error occurred within this `FeatureGraph`.
    FeatureGraphInternalError(String),
}

impl Error {
    pub(crate) fn command_error(err: cargo_metadata::Error) -> Self {
        Error::CommandError(Box::new(err))
    }

    pub(crate) fn unknown_feature_id(feature_id: FeatureId<'_>) -> Self {
        let (package_id, feature) = feature_id.into();
        Error::UnknownFeatureId(package_id, feature)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError(err) => write!(f, "Error while executing 'cargo metadata': {}", err),
            MetadataParseError(err) => write!(
                f,
                "Error while parsing 'cargo metadata' JSON output: {}",
                err
            ),
            MetadataSerializeError(err) => write!(
                f,
                "Error while serializing 'cargo metadata' JSON output: {}",
                err
            ),
            PackageGraphConstructError(msg) => {
                write!(f, "Error while computing package graph: {}", msg)
            }
            UnknownPackageId(id) => write!(f, "Unknown package ID: {}", id),
            UnknownFeatureId(package_id, feature) => match feature {
                Some(feature) => write!(f, "Unknown feature ID: '{}' '{}'", package_id, feature),
                None => write!(f, "Unknown feature ID: '{}' (base)", package_id),
            },
            UnknownWorkspacePath(path) => write!(f, "Unknown workspace path: {}", path.display()),
            UnknownWorkspaceName(name) => write!(f, "Unknown workspace package name: {}", name),
            CargoSetError(msg) => write!(f, "Error while computing Cargo set: {}", msg),
            PackageGraphInternalError(msg) => write!(f, "Internal error in package graph: {}", msg),
            FeatureGraphInternalError(msg) => write!(f, "Internal error in feature graph: {}", msg),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            MetadataParseError(err) => Some(err),
            MetadataSerializeError(err) => Some(err),
            CommandError(err) => Some(err.as_ref()),
            PackageGraphConstructError(_) => None,
            UnknownPackageId(_) => None,
            UnknownFeatureId(_, _) => None,
            UnknownWorkspacePath(_) => None,
            UnknownWorkspaceName(_) => None,
            CargoSetError(_) => None,
            PackageGraphInternalError(_) => None,
            FeatureGraphInternalError(_) => None,
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
