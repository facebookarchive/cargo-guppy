// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use cfg_expr::{target_lexicon::Triple, TargetPredicate};
use std::{borrow::Cow, error, fmt, str::FromStr};

/// A single, specific target, uniquely identified by a triple.
///
/// A `SingleTarget` may be constructed through `new` or the `FromStr` implementation.
///
/// Every [`Platform`](crate::Platform) has one of these, and an evaluation
/// [`TargetSpec`](crate::TargetSpec) may be backed by one of these as well.
///
/// # Examples
///
/// ```
/// ```
#[derive(Clone, Debug)]
pub struct SingleTarget {
    /// The triple string, for example `x86_64-unknown-linux-gnu`. This can be provided at
    /// construction time (preferred), or derived from the `Triple` if not.
    triple_str: Cow<'static, str>,

    /// The triple used for comparisons.
    lexicon_triple: Triple,
}

impl SingleTarget {
    /// Creates a new `SingleTarget` from a triple string.
    pub fn new(triple_str: impl Into<Cow<'static, str>>) -> Result<Self, SingleTargetParseError> {
        let triple_str = triple_str.into();
        match triple_str.parse::<Triple>() {
            Ok(lexicon_triple) => Ok(Self {
                triple_str,
                lexicon_triple,
            }),
            Err(lexicon_err) => Err(SingleTargetParseError {
                triple_str,
                lexicon_err,
            }),
        }
    }

    /// Returns the triple string corresponding to this target.
    pub fn triple_str(&self) -> &str {
        &self.triple_str
    }

    // Use cfg-expr's target matcher.
    pub(crate) fn matches(&self, target: &TargetPredicate) -> bool {
        target.matches(&self.lexicon_triple)
    }
}

impl FromStr for SingleTarget {
    type Err = SingleTargetParseError;

    fn from_str(triple_str: &str) -> Result<Self, Self::Err> {
        match triple_str.parse::<Triple>() {
            Ok(lexicon_triple) => Ok(Self {
                triple_str: triple_str.to_owned().into(),
                lexicon_triple,
            }),
            Err(lexicon_err) => Err(SingleTargetParseError {
                triple_str: triple_str.to_owned().into(),
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
    triple_str: Cow<'static, str>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use target_lexicon::*;

    #[test]
    fn test_parse() {
        let target =
            SingleTarget::new("x86_64-pc-darwin").expect("this triple is known to target-lexicon");

        let expected_triple = Triple {
            architecture: Architecture::X86_64,
            vendor: Vendor::Pc,
            operating_system: OperatingSystem::Darwin,
            environment: Environment::Unknown,
            binary_format: BinaryFormat::Macho,
        };
        assert_eq!(
            target.lexicon_triple, expected_triple,
            "lexicon triple matched correctly"
        );
    }
}
