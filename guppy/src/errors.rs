// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

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
