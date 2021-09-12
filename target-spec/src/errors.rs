// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Errors returned by `target-spec`.

use std::{borrow::Cow, error, fmt};

/// An error that happened during `target-spec` parsing or evaluation.
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// A `cfg()` expression was invalid and could not be parsed.
    InvalidExpression(ExpressionParseError),
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
            Error::InvalidExpression(_) => write!(f, "invalid cfg() expression"),
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
            Error::InvalidExpression(err) => Some(err),
            Error::UnknownTargetTriple(err) => Some(err),
            Error::UnknownPlatformTriple(err) => Some(err),
            Error::UnknownPredicate(_) => None,
        }
    }
}

/// An error returned in case a `TargetExpression` cannot be parsed.
#[derive(Debug, PartialEq)]
pub struct ExpressionParseError {
    inner: cfg_expr::ParseError,
}

impl ExpressionParseError {
    pub(crate) fn new(inner: cfg_expr::ParseError) -> Self {
        Self { inner }
    }
}

impl fmt::Display for ExpressionParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "expression parse error")
    }
}

impl error::Error for ExpressionParseError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(&self.inner)
    }
}

/// An error returned while parsing a single target.
///
/// This is caused by a triple not being understood by either `cfg-expr` or `target-lexicon`.
#[derive(Debug, PartialEq)]
pub struct SingleTargetParseError {
    triple_str: Cow<'static, str>,
    lexicon_err: cfg_expr::target_lexicon::ParseError,
}

impl SingleTargetParseError {
    pub(crate) fn new(
        triple_str: Cow<'static, str>,
        lexicon_err: cfg_expr::target_lexicon::ParseError,
    ) -> Self {
        Self {
            triple_str,
            lexicon_err,
        }
    }

    /// Returns the triple string that could not be parsed.
    pub fn triple_str(&self) -> &str {
        &self.triple_str
    }
}

impl fmt::Display for SingleTargetParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown triple string: {}", self.triple_str)
    }
}

impl error::Error for SingleTargetParseError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(&self.lexicon_err)
    }
}
