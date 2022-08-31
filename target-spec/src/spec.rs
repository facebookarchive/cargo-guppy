// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{errors::ExpressionParseError, Error, Platform, Triple};
use cfg_expr::{Expression, Predicate};
use std::{borrow::Cow, str::FromStr, sync::Arc};

/// A parsed target specification or triple, as found in a `Cargo.toml` file.
///
/// ## Examples
///
/// ```
/// use target_spec::{Platform, TargetFeatures, TargetSpec};
///
/// let i686_windows = Platform::new("i686-pc-windows-gnu", TargetFeatures::Unknown).unwrap();
/// let x86_64_mac = Platform::new("x86_64-apple-darwin", TargetFeatures::none()).unwrap();
/// let i686_linux = Platform::new(
///     "i686-unknown-linux-gnu",
///     TargetFeatures::features(["sse2"].iter().copied()),
/// ).unwrap();
///
/// let spec: TargetSpec = "cfg(any(windows, target_arch = \"x86_64\"))".parse().unwrap();
/// assert_eq!(spec.eval(&i686_windows), Some(true), "i686 Windows");
/// assert_eq!(spec.eval(&x86_64_mac), Some(true), "x86_64 MacOS");
/// assert_eq!(spec.eval(&i686_linux), Some(false), "i686 Linux (should not match)");
///
/// let spec: TargetSpec = "cfg(any(target_feature = \"sse2\", target_feature = \"sse\"))".parse().unwrap();
/// assert_eq!(spec.eval(&i686_windows), None, "i686 Windows features are unknown");
/// assert_eq!(spec.eval(&x86_64_mac), Some(false), "x86_64 MacOS matches no features");
/// assert_eq!(spec.eval(&i686_linux), Some(true), "i686 Linux matches some features");
/// ```
#[derive(Clone, Debug)]
pub enum TargetSpec {
    /// An exact target parsed from a triple.
    ///
    /// Parsed from strings like `"i686-pc-windows-gnu"`.
    Triple(Triple),

    /// A complex expression.
    ///
    /// Parsed from strings like `"cfg(any(windows, target_arch = \"x86_64\"))"`.
    Expression(TargetExpression),
}

impl TargetSpec {
    /// Creates a new target from a string.
    pub fn new(input: impl Into<Cow<'static, str>>) -> Result<Self, Error> {
        let input = input.into();

        if input.starts_with("cfg(") {
            Ok(TargetSpec::Expression(TargetExpression::new(&input)?))
        } else {
            match Triple::new(input) {
                Ok(triple) => Ok(TargetSpec::Triple(triple)),
                Err(parse_err) => Err(Error::UnknownTargetTriple(parse_err)),
            }
        }
    }

    /// Evaluates this specification against the given platform.
    ///
    /// Returns `Some(true)` if there's a match, `Some(false)` if there's none, or `None` if the
    /// result of the evaluation is unknown (typically found if target features are involved).
    #[inline]
    pub fn eval(&self, platform: &Platform) -> Option<bool> {
        match self {
            TargetSpec::Triple(triple) => Some(triple.eval(platform)),
            TargetSpec::Expression(expr) => expr.eval(platform),
        }
    }
}

impl FromStr for TargetSpec {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::new(input.to_owned())
    }
}

/// A target expression.
///
/// Parsed from a string beginning with `cfg(`.
#[derive(Clone, Debug)]
pub struct TargetExpression {
    inner: Arc<Expression>,
}

impl TargetExpression {
    /// Creates a new `TargetExpression` from a string beginning with `cfg(`.
    ///
    /// Returns an error if the string could not be parsed, or if the string contains a predicate
    /// that wasn't understood by `target-spec`.
    pub fn new(input: &str) -> Result<Self, Error> {
        let expr = Expression::parse(input)
            .map_err(|err| Error::InvalidExpression(ExpressionParseError::new(err)))?;
        Ok(Self {
            inner: Arc::new(expr),
        })
    }

    /// Returns the string that was parsed into `self`.
    #[inline]
    pub fn expression_str(&self) -> &str {
        self.inner.original()
    }

    /// Evaluates this expression against the given platform.
    ///
    /// Returns `Some(true)` if there's a match, `Some(false)` if there's none, or `None` if the
    /// result of the evaluation is unknown (typically found if target features are involved).
    pub fn eval(&self, platform: &Platform) -> Option<bool> {
        self.inner.eval(|pred| {
            match pred {
                Predicate::Target(target) => Some(platform.triple().matches(target)),
                Predicate::TargetFeature(feature) => platform.target_features().matches(feature),
                Predicate::Test | Predicate::DebugAssertions | Predicate::ProcMacro => {
                    // Known families that always evaluate to false. See
                    // https://docs.rs/cargo-platform/0.1.1/src/cargo_platform/lib.rs.html#76.
                    Some(false)
                }
                Predicate::Feature(_) => {
                    // NOTE: This is not supported by Cargo which always evaluates this to false. See
                    // https://github.com/rust-lang/cargo/issues/7442 for more details.
                    Some(false)
                }
                Predicate::Flag(flag) => {
                    // This returns false by default but true in some cases.
                    Some(platform.has_flag(flag))
                }
                Predicate::KeyValue { .. } => {
                    // This is always interpreted by Cargo as false.
                    Some(false)
                }
            }
        })
    }
}

impl FromStr for TargetExpression {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::new(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cfg_expr::{
        targets::{Family, Os},
        Predicate, TargetPredicate,
    };

    #[test]
    fn test_triple() {
        let res = TargetSpec::new("x86_64-apple-darwin");
        assert!(matches!(
            res,
            Ok(TargetSpec::Triple(triple)) if triple.as_str() == "x86_64-apple-darwin"
        ));
    }

    #[test]
    fn test_single() {
        let expr = match TargetSpec::new("cfg(windows)").unwrap() {
            TargetSpec::Triple(triple) => {
                panic!("expected expression, got triple: {:?}", triple)
            }
            TargetSpec::Expression(expr) => expr,
        };
        assert_eq!(
            expr.inner.predicates().collect::<Vec<_>>(),
            vec![Predicate::Target(TargetPredicate::Family(Family::windows))],
        );
    }

    #[test]
    fn test_not() {
        assert!(matches!(
            TargetSpec::new("cfg(not(windows))"),
            Ok(TargetSpec::Expression(_))
        ));
    }

    #[test]
    fn test_testequal() {
        let expr = match TargetSpec::new("cfg(target_os = \"windows\")").unwrap() {
            TargetSpec::Triple(triple) => {
                panic!("expected spec, got triple: {:?}", triple)
            }
            TargetSpec::Expression(expr) => expr,
        };

        assert_eq!(
            expr.inner.predicates().collect::<Vec<_>>(),
            vec![Predicate::Target(TargetPredicate::Os(Os::windows))],
        );
    }

    #[test]
    fn test_unknown_triple() {
        // This used to be "x86_64-pc-darwin", but target-lexicon can parse that.
        let err = TargetSpec::new("cannotbeknown").expect_err("unknown triple");
        assert!(matches!(
            err,
            Error::UnknownTargetTriple(parse_err) if parse_err.triple_str() == "cannotbeknown"
        ));
    }

    #[test]
    fn test_unknown_flag() {
        let expr = match TargetSpec::new("cfg(foo)").unwrap() {
            TargetSpec::Triple(triple) => {
                panic!("expected spec, got triple: {:?}", triple)
            }
            TargetSpec::Expression(expr) => expr,
        };

        assert_eq!(
            expr.inner.predicates().collect::<Vec<_>>(),
            vec![Predicate::Flag("foo")],
        );
    }

    #[test]
    fn test_unknown_predicate() {
        let expr = match TargetSpec::new("cfg(bogus_key = \"bogus_value\")")
            .expect("unknown predicate should parse")
        {
            TargetSpec::Triple(triple) => {
                panic!("expected spec, got triple: {:?}", triple)
            }
            TargetSpec::Expression(expr) => expr,
        };
        assert_eq!(
            expr.inner.predicates().collect::<Vec<_>>(),
            vec![Predicate::KeyValue {
                key: "bogus_key",
                val: "bogus_value"
            }],
        );

        let platform = Platform::current().unwrap();
        // This should always evaluate to false.
        assert_eq!(expr.eval(&platform), Some(false));

        let expr = TargetSpec::new("cfg(not(bogus_key = \"bogus_value\"))")
            .expect("unknown predicate should parse");
        // This is a cfg(not()), so it should always evaluate to true.
        assert_eq!(expr.eval(&platform), Some(true));
    }

    #[test]
    fn test_extra() {
        let res = TargetSpec::new("cfg(unix)this-is-extra");
        res.expect_err("extra content at the end");
    }

    #[test]
    fn test_incomplete() {
        // This fails because the ) at the end is missing.
        let res = TargetSpec::new("cfg(not(unix)");
        res.expect_err("missing ) at the end");
    }
}
