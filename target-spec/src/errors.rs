// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::error;
use std::fmt;

/// An error that happened during `target-spec` parsing or evaluation.
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// A `cfg()` expression was invalid and could not be parsed.
    InvalidCfg(cfg_expr::ParseError),
    /// The provided target triple (in the position that a `cfg()` expression would be) was unknown.
    UnknownTargetTriple(String),
    /// The provided platform triple was unknown.
    UnknownPlatformTriple(String),
    /// The provided `cfg()` expression parsed correctly, but it had an unknown predicate.
    UnknownPredicate(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidCfg(_) => write!(f, "invalid cfg() expression"),
            Error::UnknownTargetTriple(triple) => write!(f, "unknown target triple: {}", triple),
            Error::UnknownPlatformTriple(triple) => {
                write!(f, "unknown platform triple: {}", triple)
            }
            Error::UnknownPredicate(pred) => {
                write!(f, "cfg() expression has unknown predicate: {}", pred)
            }
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::InvalidCfg(err) => Some(err),
            Error::UnknownTargetTriple(_) => None,
            Error::UnknownPlatformTriple(_) => None,
            Error::UnknownPredicate(_) => None,
        }
    }
}
