// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use cfg_expr::{
    target_lexicon::Triple,
    targets::{get_builtin_target_by_triple, TargetInfo},
    TargetPredicate,
};
use std::{borrow::Cow, error, fmt, str::FromStr};

/// Support for creating custom platforms.
///
/// This module re-exports parts of the `cfg_expr` and `target_lexicon` dependencies to allow
/// creating platforms unknown to `target-spec` or `rustc` by default.
pub mod custom_platforms {
    /// Parts of the `cfg_expr` dependency.
    pub mod cfg_expr {
        #[doc(inline)]
        pub use cfg_expr::targets::{Arch, Endian, Env, Family, Os, TargetInfo, Triple, Vendor};
    }

    /// The `target_lexicon` dependency.
    pub use ::cfg_expr::target_lexicon;
}

/// A single, specific target, uniquely identified by a triple.
///
/// A `SingleTarget` may be constructed through the `From` implementations, or by parsing a
/// triple string.
///
/// A target can be backed by either:
/// * a [`cfg_expr::targets::TargetInfo`], or
/// * a [`target_lexicon::Triple`](`target_lexicon::Triple`).
///
/// Every [`Platform`](crate::Platform) has one of these, and an evaluation
/// [`TargetSpec`](crate::TargetSpec) may be backed by one of these as well.
#[derive(Clone, Debug)]
pub enum SingleTarget {
    /// A target backed by a [`cfg_expr::targets::TargetInfo`].
    ///
    /// When resolving a triple, a `TargetInfo` is preferred.
    TargetInfo(Cow<'static, TargetInfo>),

    /// A [`target_lexicon::Triple`]. This is a fallback option that is looser, and may be used if a
    /// `TargetInfo` isn't aware of a specific triple.
    Lexicon {
        /// The triple string, for example `x86_64-unknown-linux-gnu`. This can be provided at
        /// construction time (preferred), or derived from the `Triple` if not.
        triple_str: String,

        /// The triple used for comparisons.
        lexicon_triple: Triple,
    },
}

impl SingleTarget {
    /// Returns the triple string corresponding to this target.
    pub fn triple_str(&self) -> &str {
        match self {
            SingleTarget::TargetInfo(target_info) => target_info.triple.as_str(),
            SingleTarget::Lexicon { triple_str, .. } => triple_str,
        }
    }

    // Use cfg-expr's target matcher.
    pub(crate) fn matches(&self, target: &TargetPredicate) -> bool {
        match self {
            SingleTarget::TargetInfo(target_info) => target.matches(target_info.as_ref()),
            SingleTarget::Lexicon { lexicon_triple, .. } => target.matches(lexicon_triple),
        }
    }
}

impl From<&'static TargetInfo> for SingleTarget {
    fn from(target_info: &'static TargetInfo) -> Self {
        SingleTarget::TargetInfo(Cow::Borrowed(target_info))
    }
}

impl From<TargetInfo> for SingleTarget {
    fn from(target_info: TargetInfo) -> Self {
        SingleTarget::TargetInfo(Cow::Owned(target_info))
    }
}

impl From<Triple> for SingleTarget {
    fn from(lexicon_triple: Triple) -> Self {
        SingleTarget::Lexicon {
            triple_str: format!("{}", lexicon_triple),
            lexicon_triple,
        }
    }
}

impl FromStr for SingleTarget {
    type Err = SingleTargetParseError;

    fn from_str(triple_str: &str) -> Result<Self, Self::Err> {
        if let Some(target_info) = get_builtin_target_by_triple(triple_str) {
            return Ok(SingleTarget::TargetInfo(Cow::Borrowed(target_info)));
        }
        match triple_str.parse::<Triple>() {
            Ok(lexicon_triple) => Ok(SingleTarget::Lexicon {
                triple_str: triple_str.into(),
                lexicon_triple,
            }),
            Err(lexicon_err) => Err(SingleTargetParseError {
                triple_str: triple_str.into(),
                lexicon_err,
            }),
        }
    }
}

/// An error returned while parsing a single target.
///
/// This is caused by a triple not being understood by either `cfg-expr` or `target-lexicon`.
#[derive(Debug, PartialEq)]
pub struct SingleTargetParseError {
    triple_str: Box<str>,
    lexicon_err: cfg_expr::target_lexicon::ParseError,
}

impl SingleTargetParseError {
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
