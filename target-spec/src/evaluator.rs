// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::parser::ParseError;
use crate::types::{Atom, Expr, TargetEnum};
use crate::TargetSpec;
use platforms::{target::OS, Platform};
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

/// Evaluates the given spec against the provided target and returns true on a successful match.
///
/// For more information, see the crate-level documentation.
pub fn eval(spec_or_triple: &str, platform: &str) -> Result<bool, EvalError> {
    let target_spec = spec_or_triple
        .parse::<TargetSpec>()
        .map_err(EvalError::InvalidSpec)?;
    target_spec.eval(platform)
}

pub(crate) fn eval_target(target: &TargetEnum, platform: &str) -> Result<bool, EvalError> {
    match platforms::find(platform) {
        None => Err(EvalError::TargetNotFound),
        Some(platform) => match target {
            TargetEnum::Triple(ref triple) => Ok(platform.target_triple == triple),
            TargetEnum::Spec(ref expr) => eval_expr(expr, &platform),
        },
    }
}

fn eval_expr(spec: &Expr, platform: &Platform) -> Result<bool, EvalError> {
    match *spec {
        Expr::Any(ref exprs) => {
            for e in exprs {
                let res = eval_expr(e, platform);
                match res {
                    Ok(true) => return Ok(true),
                    Ok(false) => continue,
                    Err(e) => return Err(e),
                };
            }
            Ok(false)
        }
        Expr::All(ref exprs) => {
            for e in exprs {
                let res = eval_expr(e, platform);
                match res {
                    Ok(true) => continue,
                    Ok(false) => return Ok(false),
                    Err(e) => return Err(e),
                }
            }
            Ok(true)
        }
        Expr::Not(ref expr) => eval_expr(expr, platform).map(|b| !b),
        // target_family can be either unix or windows
        Expr::TestSet(Atom::Ident(ref family)) => match family.as_str() {
            "windows" => Ok(platform.target_os == OS::Windows),
            "unix" => Ok(platform.target_os == OS::Linux || platform.target_os == OS::MacOS),
            _ => Err(EvalError::UnknownOption(family.clone())),
        },
        // supports only target_os currently
        Expr::TestEqual((Atom::Ident(ref name), Atom::Value(ref value))) => {
            if name == "target_os" {
                Ok(value == platform.target_os.as_str())
            } else if name == "target_env" {
                Ok(value == platform.target_env.map(|e| e.as_str()).unwrap_or(""))
            } else if name == "target_arch" {
                Ok(value == platform.target_arch.as_str())
            } else if name == "target_vendor" {
                // hack for ring's wasm support
                Ok(value == "unknown")
            } else if name == "feature" {
                // NOTE: This is not supported by Cargo which always evaluates
                // this to false. See
                // https://github.com/rust-lang/cargo/issues/7442 for more details.
                Ok(false)
            } else {
                Err(EvalError::UnknownOption(name.clone()))
            }
        }
        _ => unreachable!("can't get here"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows() {
        assert_eq!(eval("cfg(windows)", "x86_64-pc-windows-msvc"), Ok(true),);
    }

    #[test]
    fn test_not_target_os() {
        assert_eq!(
            eval(
                "cfg(not(target_os = \"windows\"))",
                "x86_64-unknown-linux-gnu"
            ),
            Ok(true),
        );
    }

    #[test]
    fn test_not_target_os_false() {
        assert_eq!(
            eval(
                "cfg(not(target_os = \"windows\"))",
                "x86_64-pc-windows-msvc"
            ),
            Ok(false),
        );
    }

    #[test]
    fn test_exact_triple() {
        assert_eq!(
            eval("x86_64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"),
            Ok(true),
        );
    }

    #[test]
    fn test_redox() {
        assert_eq!(
            eval(
                "cfg(any(unix, target_os = \"redox\"))",
                "x86_64-unknown-linux-gnu"
            ),
            Ok(true),
        );
    }
}
