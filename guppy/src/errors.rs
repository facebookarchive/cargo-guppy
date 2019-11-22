// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use cargo_metadata::{Error as MetadataError, PackageId as MetadataPackageId};
use serde_json;
use std::error;
use std::fmt;
use std::io;

use Error::*;

#[derive(Debug)]
pub enum Error {
    ConfigIoError(io::Error),
    ConfigParseError(toml::de::Error),
    CommandError(MetadataError),
    MetadataParseError(serde_json::Error),
    DepGraphError(String),
    DepGraphUnknownPackageId(MetadataPackageId),
    DepGraphInternalError(String),
    PackageIdParseError(MetadataPackageId, String),
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
            DepGraphError(msg) => write!(f, "Error while computing dependency graph: {}", msg),
            DepGraphUnknownPackageId(id) => write!(f, "Unknown package ID: {}", id),
            DepGraphInternalError(msg) => write!(f, "Internal error in dependency graph: {}", msg),
            PackageIdParseError(id, msg) => write!(f, "Error parsing package ID '{}': {}", id, msg),
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
            DepGraphError(_) => None,
            DepGraphUnknownPackageId(_) => None,
            DepGraphInternalError(_) => None,
            PackageIdParseError(_, _) => None,
        }
    }
}
