// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::SingleTargetParseError;
use std::{error, fmt};

/// An error that happened during `target-spec` parsing or evaluation.
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// A `cfg()` expression was invalid and could not be parsed.
    InvalidCfg(cfg_expr::ParseError),
    /// The provided target triple (in the position that a `cfg()` expression would be) was unknown.
    UnknownTargetTriple(SingleTargetParseError),
    /// The provided platform triple was unknown.
    UnknownPlatformTriple(SingleTargetParseError),
    /// The provided `cfg()` expression parsed correctly, but it had an unknown predicate.
    UnknownPredicate(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidCfg(_) => write!(f, "invalid cfg() expression"),
            Error::UnknownTargetTriple(_) => write!(f, "unknown target triple"),
            Error::UnknownPlatformTriple(_) => {
                write!(f, "unknown platform triple")
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
            Error::UnknownTargetTriple(err) => Some(err),
            Error::UnknownPlatformTriple(err) => Some(err),
            Error::UnknownPredicate(_) => None,
        }
    }
}
