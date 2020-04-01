// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::Target;
use cfg_expr::targets::get_target_by_triple;
use cfg_expr::Expression;
use std::sync::Arc;
use std::{error, fmt};

/// An error that occurred while attempting to parse a target specification.
#[derive(Clone, Debug, PartialEq)]
pub struct ParseError(String);

impl ParseError {
    pub(crate) fn new(err: cfg_expr::ParseError<'_>) -> Self {
        ParseError(format!("{}", err))
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error parsing target spec: {}", self.0)
    }
}

impl error::Error for ParseError {}

/// Parses this expression into a `Target` instance.
pub(crate) fn parse(input: &str) -> Result<Target, ParseError> {
    if input.starts_with("cfg(") {
        Ok(Target::Spec(Arc::new(
            Expression::parse(input).map_err(ParseError::new)?,
        )))
    } else {
        Ok(Target::TargetInfo(get_target_by_triple(input).ok_or_else(
            || ParseError(format!("unrecognized target triple '{}'", input)),
        )?))
    }
}
