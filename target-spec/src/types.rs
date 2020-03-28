// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::parser::parse;
use crate::platform::Platform;
use crate::{eval_target, EvalError, ParseError};
use cfg_expr::targets::TargetInfo;
use cfg_expr::Expression;
use std::str::FromStr;
use std::sync::Arc;

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
    target: TargetEnum,
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
            target: parse(input)?,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) enum TargetEnum {
    TargetInfo(&'static TargetInfo),
    Spec(Arc<Expression>),
}
