// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::error;
use std::fmt;
use std::io;

use Error::*;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    LockfileParseError(toml::de::Error),
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
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Io(err) => Some(err),
            LockfileParseError(err) => Some(err),
        }
    }
}
