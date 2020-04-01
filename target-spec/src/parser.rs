// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{eval_target, EvalError, Platform};
use cfg_expr::targets::{get_target_by_triple, TargetInfo};
use cfg_expr::Expression;
use std::str::FromStr;
use std::sync::Arc;
use std::{error, fmt};

/// A parsed target specification or triple, as found in a `Cargo.toml` file.
///
/// Use the `FromStr` implementation or `str::parse` to obtain an instance.
///
/// ## Examples
///
/// ```
/// use target_spec::{Platform, TargetFeatures, TargetSpec};
///
/// let i686_windows = Platform::new("i686-pc-windows-gnu", TargetFeatures::Unknown).unwrap();
/// let x86_64_mac = Platform::new("x86_64-apple-darwin", TargetFeatures::none()).unwrap();
/// let i686_linux = Platform::new("i686-unknown-linux-gnu", TargetFeatures::features(&["sse2"])).unwrap();
///
/// let spec: TargetSpec = "cfg(any(windows, target_arch = \"x86_64\"))".parse().unwrap();
/// assert_eq!(spec.eval(&i686_windows).unwrap(), Some(true), "i686 Windows");
/// assert_eq!(spec.eval(&x86_64_mac).unwrap(), Some(true), "x86_64 MacOS");
/// assert_eq!(spec.eval(&i686_linux).unwrap(), Some(false), "i686 Linux (should not match)");
///
/// let spec: TargetSpec = "cfg(any(target_feature = \"sse2\", target_feature = \"sse\"))".parse().unwrap();
/// assert_eq!(spec.eval(&i686_windows).unwrap(), None, "i686 Windows features are unknown");
/// assert_eq!(spec.eval(&x86_64_mac).unwrap(), Some(false), "x86_64 MacOS matches no features");
/// assert_eq!(spec.eval(&i686_linux).unwrap(), Some(true), "i686 Linux matches some features");
/// ```
#[derive(Clone, Debug)]
pub struct TargetSpec {
    target: Target,
}

impl TargetSpec {
    /// Evaluates this specification against the given platform triple, defaulting to accepting all
    /// target features.
    #[inline]
    pub fn eval(&self, platform: &Platform<'_>) -> Result<Option<bool>, EvalError> {
        eval_target(&self.target, platform)
    }
}

impl FromStr for TargetSpec {
    type Err = ParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            target: Target::parse(input)?,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Target {
    TargetInfo(&'static TargetInfo),
    Spec(Arc<Expression>),
}

impl Target {
    /// Parses this expression into a `Target` instance.
    fn parse(input: &str) -> Result<Target, ParseError> {
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
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use cfg_expr::targets::{Family, Os};
    use cfg_expr::{Predicate, TargetPredicate};

    #[test]
    fn test_triple() {
        let res = Target::parse("x86_64-apple-darwin");
        assert!(matches!(
            res,
            Ok(Target::TargetInfo(target_info)) if target_info.triple == "x86_64-apple-darwin"
        ));
    }

    #[test]
    fn test_single() {
        let expr = match Target::parse("cfg(windows)").unwrap() {
            Target::TargetInfo(target_info) => {
                panic!("expected spec, got target info: {:?}", target_info)
            }
            Target::Spec(expr) => expr,
        };
        assert_eq!(
            expr.predicates().collect::<Vec<_>>(),
            vec![Predicate::Target(TargetPredicate::Family(Some(
                Family::windows
            )))],
        );
    }

    #[test]
    fn test_not() {
        assert!(matches!(
            Target::parse("cfg(not(windows))"),
            Ok(Target::Spec(_))
        ));
    }

    #[test]
    fn test_testequal() {
        let expr = match Target::parse("cfg(target_os = \"windows\")").unwrap() {
            Target::TargetInfo(target_info) => {
                panic!("expected spec, got target info: {:?}", target_info)
            }
            Target::Spec(expr) => expr,
        };

        assert_eq!(
            expr.predicates().collect::<Vec<_>>(),
            vec![Predicate::Target(TargetPredicate::Os(Some(Os::windows)))],
        );
    }

    #[test]
    fn test_extra() {
        let res = Target::parse("cfg(unix)this-is-extra");
        res.expect_err("extra content at the end");
    }

    #[test]
    fn test_incomplete() {
        // This fails because the ) at the end is missing.
        let res = Target::parse("cfg(not(unix)");
        res.expect_err("missing ) at the end");
    }
}
