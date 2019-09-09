use cargo_metadata::{Error as MetadataError, PackageId as MetadataPackageId};
use std::error;
use std::fmt;
use std::io;

use Error::*;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    InvalidInput,
    CommandError(MetadataError),
    DepGraphError(String),
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
            InvalidInput => write!(f, "Failed to read Cargo.lock file"),
            CommandError(err) => write!(f, "Error while executing 'cargo metadata': {}", err),
            DepGraphError(msg) => write!(f, "Error while computing dependency graph: {}", msg),
            PackageIdParseError(id, msg) => write!(f, "Error parsing package ID '{}': {}", id, msg),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Io(err) => Some(err),
            InvalidInput => None,
            CommandError(_) => None,
            DepGraphError(_) => None,
            PackageIdParseError(_, _) => None,
        }
    }
}
