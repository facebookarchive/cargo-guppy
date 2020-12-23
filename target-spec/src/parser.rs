// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{custom_platforms::TargetInfo, eval_target, Error, Platform};
use cfg_expr::{targets::get_builtin_target_by_triple, Expression, Predicate};
use std::{str::FromStr, sync::Arc};

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
pub struct TargetSpec<'a> {
    target: Target<'a>,
}

impl<'a> TargetSpec<'a> {
    /// Creates a new exact, custom target spec to match against.
    ///
    /// Note that this is for an *exact* target spec, similar to a triple specified, not an
    /// expression like `cfg(windows)`.
    ///
    /// Custom platforms are often found in embedded and similar environments. For built-in
    /// platforms, the `FromStr` implementation is recommended instead.
    pub fn custom(target_info: &'a TargetInfo<'a>) -> Self {
        Self {
            target: Target::TargetInfo(target_info),
        }
    }

    /// Evaluates this specification against the given platform.
    ///
    /// Returns `Some(true)` if there's a match, `Some(false)` if there's none, or `None` if the
    /// result of the evaluation is unknown (typically found if target features are involved).
    #[inline]
    pub fn eval(&self, platform: &Platform<'_>) -> Option<bool> {
        eval_target(&self.target, platform)
    }
}

impl FromStr for TargetSpec<'static> {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            target: Target::parse(input)?,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Target<'a> {
    TargetInfo(&'a TargetInfo<'a>),
    Spec(Arc<Expression>),
}

impl Target<'static> {
    /// Parses this expression into a `Target` instance.
    fn parse(input: &str) -> Result<Self, Error> {
        if input.starts_with("cfg(") {
            let expr = Expression::parse(input).map_err(Error::InvalidCfg)?;
            Self::verify_expr(expr)
        } else {
            Ok(Target::TargetInfo(
                get_builtin_target_by_triple(input)
                    .ok_or_else(|| Error::UnknownTargetTriple(input.to_string()))?,
            ))
        }
    }
}

impl<'a> Target<'a> {
    /// Verify this `cfg()` expression.
    fn verify_expr(expr: Expression) -> Result<Self, Error> {
        // Error out on unknown key-value pairs. Everything else is recognized (though
        // DebugAssertions/ProcMacro etc always returns false, and flags return false by default).
        for pred in expr.predicates() {
            if let Predicate::KeyValue { key, .. } = pred {
                return Err(Error::UnknownPredicate(key.to_string()));
            }
        }
        Ok(Target::Spec(Arc::new(expr)))
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
            vec![Predicate::Target(TargetPredicate::Family(Family::windows))],
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
            vec![Predicate::Target(TargetPredicate::Os(Os::windows))],
        );
    }

    #[test]
    fn test_unknown_triple() {
        let err = Target::parse("x86_64-pc-darwin").expect_err("unknown triple");
        assert_eq!(
            err,
            Error::UnknownTargetTriple("x86_64-pc-darwin".to_string())
        );
    }

    #[test]
    fn test_unknown_flag() {
        let expr = match Target::parse("cfg(foo)").unwrap() {
            Target::TargetInfo(target_info) => {
                panic!("expected spec, got target info: {:?}", target_info)
            }
            Target::Spec(expr) => expr,
        };

        assert_eq!(
            expr.predicates().collect::<Vec<_>>(),
            vec![Predicate::Flag("foo")],
        );
    }

    #[test]
    fn test_unknown_predicate() {
        let err = Target::parse("cfg(bogus_key = \"bogus_value\")").expect_err("unknown predicate");
        assert_eq!(err, Error::UnknownPredicate("bogus_key".to_string()));
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
