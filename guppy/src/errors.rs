// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use cargo_metadata::{Error as MetadataError, PackageId as MetadataPackageId};
use std::error;
use std::fmt;
use std::io;

use Error::*;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    LockfileParseError(toml::de::Error),
    ConfigIoError(io::Error),
    ConfigParseError(toml::de::Error),
    CommandError(MetadataError),
    DepGraphError(String),
    DepGraphUnknownPackageId(MetadataPackageId),
    DepGraphInternalError(String),
    PackageIdParseError(MetadataPackageId, String),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Io(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Io(err) => write!(f, "{}", err),
            LockfileParseError(err) => write!(f, "Error while parsing lockfile: {}", err),
            ConfigIoError(err) => write!(f, "Error while reading config file: {}", err),
            ConfigParseError(err) => write!(f, "Error while parsing config file: {}", err),
            CommandError(err) => write!(f, "Error while executing 'cargo metadata': {}", err),
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
            Io(err) => Some(err),
            LockfileParseError(err) => Some(err),
            ConfigIoError(err) => Some(err),
            ConfigParseError(err) => Some(err),
            CommandError(_) => None,
            DepGraphError(_) => None,
            DepGraphUnknownPackageId(_) => None,
            DepGraphInternalError(_) => None,
            PackageIdParseError(_, _) => None,
        }
    }
}
