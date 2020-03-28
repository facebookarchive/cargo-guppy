// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::parser::parse_impl;
use crate::{eval_target, EvalError, ParseError};
use std::str::FromStr;

/// A parsed target specification or triple, as found in a `Cargo.toml` file.
///
/// Use the `FromStr` implementation or `str::parse` to obtain an instance.
///
/// ## Examples
///
/// ```
/// use target_spec::TargetSpec;
///
/// let spec: TargetSpec = "cfg(any(windows, target_arch = \"x86_64\"))".parse().unwrap();
/// assert!(spec.eval("i686-pc-windows-gnu").unwrap(), "i686 Windows");
/// assert!(spec.eval("x86_64-apple-darwin").unwrap(), "x86_64 MacOS");
/// assert!(!spec.eval("i686-unknown-linux-gnu").unwrap(), "i686 Linux (should not match)");
/// ```
#[derive(Clone, Debug)]
pub struct TargetSpec {
    target: TargetEnum,
}

impl TargetSpec {
    /// Evaluates this specification against the given platform triple.
    pub fn eval(&self, platform: &str) -> Result<bool, EvalError> {
        eval_target(&self.target, platform)
    }
}

impl FromStr for TargetSpec {
    type Err = ParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match parse_impl(input) {
            Ok(target) => Ok(Self { target }),
            Err(err) => Err(ParseError(err.to_owned())),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Atom {
    Ident(String),
    Value(String),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Expr {
    Any(Vec<Expr>),
    All(Vec<Expr>),
    Not(Box<Expr>),
    TestSet(Atom),
    TestEqual((Atom, Atom)),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TargetEnum {
    Triple(String),
    Spec(Expr),
}
