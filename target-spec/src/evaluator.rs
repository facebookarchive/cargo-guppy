// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::parser::ParseError;
use crate::platform::{Platform, TargetFeatures};
use crate::types::TargetEnum;
use crate::TargetSpec;
use cfg_expr::{Expression, Predicate};
use std::sync::Arc;
use std::{error, fmt};

/// An error that occurred during target evaluation.
#[derive(PartialEq)]
pub enum EvalError {
    /// An invalid target was specified.
    InvalidSpec(ParseError),
    /// The target triple was not found in the database.
    TargetNotFound,
    /// The target family wasn't recognized.
    UnknownOption(String),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            EvalError::InvalidSpec(_) => write!(f, "invalid target spec"),
            EvalError::TargetNotFound => write!(f, "target triple not found in database"),
            EvalError::UnknownOption(ref opt) => write!(f, "target family not recognized: {}", opt),
        }
    }
}

impl fmt::Debug for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <EvalError as fmt::Display>::fmt(self, f)
    }
}

impl error::Error for EvalError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            EvalError::InvalidSpec(err) => Some(err),
            EvalError::TargetNotFound | EvalError::UnknownOption(_) => None,
        }
    }
}

/// Evaluates the given spec against the provided target and returns `Some(true)` on a successful
/// match, and `Some(false)` on a failing match.
///
/// This defaults to treating target features as unknown, and returns `None` if the overall result
/// is unknown.
///
/// For more advanced uses, see `TargetSpec::eval`.
///
/// For more information, see the crate-level documentation.
pub fn eval(spec_or_triple: &str, platform: &str) -> Result<Option<bool>, EvalError> {
    let target_spec = spec_or_triple
        .parse::<TargetSpec>()
        .map_err(EvalError::InvalidSpec)?;
    match Platform::new(platform, TargetFeatures::Unknown) {
        None => Err(EvalError::TargetNotFound),
        Some(platform) => target_spec.eval(&platform),
    }
}

pub(crate) fn eval_target(
    target: &TargetEnum,
    platform: &Platform<'_>,
) -> Result<Option<bool>, EvalError> {
    match target {
        TargetEnum::TargetInfo(ref target_info) => {
            Ok(Some(platform.triple() == target_info.triple))
        }
        TargetEnum::Spec(ref expr) => eval_expr(expr, platform),
    }
}

fn eval_expr(spec: &Arc<Expression>, platform: &Platform<'_>) -> Result<Option<bool>, EvalError> {
    // Expression::eval doesn't support returning errors, so have an Option at the top to set errors
    // into.
    let mut err = None;
    let res: Option<bool> = spec.eval(|pred| {
        match pred {
            Predicate::Target(target) => Some(target.matches(platform.target_info())),
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
            Predicate::KeyValue { key, .. } => {
                err.replace(EvalError::UnknownOption((*key).to_string()));
                Some(false)
            }
            Predicate::Flag(other) => {
                // cfg_expr turns "windows" and "unix" into target families, so they don't need to
                // be handled explicitly.
                err.replace(EvalError::UnknownOption((*other).to_string()));
                Some(false)
            }
        }
    });

    match err {
        Some(err) => Err(err),
        None => Ok(res),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows() {
        assert_eq!(
            eval("cfg(windows)", "x86_64-pc-windows-msvc"),
            Ok(Some(true)),
        );
    }

    #[test]
    fn test_not_target_os() {
        assert_eq!(
            eval(
                "cfg(not(target_os = \"windows\"))",
                "x86_64-unknown-linux-gnu"
            ),
            Ok(Some(true)),
        );
    }

    #[test]
    fn test_not_target_os_false() {
        assert_eq!(
            eval(
                "cfg(not(target_os = \"windows\"))",
                "x86_64-pc-windows-msvc"
            ),
            Ok(Some(false)),
        );
    }

    #[test]
    fn test_exact_triple() {
        assert_eq!(
            eval("x86_64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"),
            Ok(Some(true)),
        );
    }

    #[test]
    fn test_redox() {
        assert_eq!(
            eval(
                "cfg(any(unix, target_os = \"redox\"))",
                "x86_64-unknown-linux-gnu"
            ),
            Ok(Some(true)),
        );
    }

    #[test]
    fn test_bogus_families() {
        // Known bogus families.
        for family in &["test", "debug_assertions", "proc_macro"] {
            let cfg = format!("cfg({})", family);
            let cfg_not = format!("cfg(not({}))", family);
            assert_eq!(eval(&cfg, "x86_64-unknown-linux-gnu"), Ok(Some(false)));
            assert_eq!(eval(&cfg_not, "x86_64-unknown-linux-gnu"), Ok(Some(true)));
        }

        // Unknown bogus families.
        for family in &["foo", "bar", "nonsense"] {
            let cfg = format!("cfg({})", family);
            let cfg_not = format!("cfg(not({}))", family);
            assert!(matches!(
                eval(&cfg, "x86_64-unknown-linux-gnu"),
                Err(EvalError::UnknownOption(_))
            ));
            assert!(matches!(
                eval(&cfg_not, "x86_64-unknown-linux-gnu"),
                Err(EvalError::UnknownOption(_))
            ));
        }
    }

    #[test]
    fn test_target_feature() {
        // target features are unknown by default.
        assert_eq!(
            eval("cfg(target_feature = \"sse\")", "x86_64-unknown-linux-gnu"),
            Ok(None),
        );
        assert_eq!(
            eval(
                "cfg(target_feature = \"atomics\")",
                "x86_64-unknown-linux-gnu",
            ),
            Ok(None),
        );
        assert_eq!(
            eval(
                "cfg(not(target_feature = \"fxsr\"))",
                "x86_64-unknown-linux-gnu",
            ),
            Ok(None),
        );

        fn eval_ext(spec: &str, platform: &str) -> Result<Option<bool>, EvalError> {
            let platform = Platform::new(platform, TargetFeatures::features(&["sse", "sse2"]))
                .expect("platform should be found");
            let spec: TargetSpec = spec.parse().unwrap();
            spec.eval(&platform)
        }

        assert_eq!(
            eval_ext("cfg(target_feature = \"sse\")", "x86_64-unknown-linux-gnu"),
            Ok(Some(true)),
        );
        assert_eq!(
            eval_ext(
                "cfg(not(target_feature = \"sse\"))",
                "x86_64-unknown-linux-gnu",
            ),
            Ok(Some(false)),
        );
        assert_eq!(
            eval_ext("cfg(target_feature = \"fxsr\")", "x86_64-unknown-linux-gnu"),
            Ok(Some(false)),
        );
        assert_eq!(
            eval_ext(
                "cfg(not(target_feature = \"fxsr\"))",
                "x86_64-unknown-linux-gnu",
            ),
            Ok(Some(true)),
        );
    }
}
