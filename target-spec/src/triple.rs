// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{errors::TripleParseError, Platform};
use cfg_expr::{target_lexicon, TargetPredicate};
use std::{borrow::Cow, cmp::Ordering, hash, str::FromStr};

/// A single, specific target, uniquely identified by a triple.
///
/// A `Triple` may be constructed through `new` or the `FromStr` implementation.
///
/// Every [`Platform`](crate::Platform) has one of these, and an evaluation
/// [`TargetSpec`](crate::TargetSpec) may be backed by one of these as well.
///
/// # Examples
///
/// ```
/// use target_spec::Triple;
///
/// // Parse a simple target.
/// let target = Triple::new("x86_64-unknown-linux-gnu").unwrap();
/// // This is not a valid triple.
/// let err = Triple::new("cannot-be-known").unwrap_err();
/// ```
#[derive(Clone, Debug)]
pub struct Triple {
    triple_str: Cow<'static, str>,
    lexicon_triple: target_lexicon::Triple,
}

impl Triple {
    /// Creates a new `Triple` from a triple string.
    pub fn new(triple_str: impl Into<Cow<'static, str>>) -> Result<Self, TripleParseError> {
        let triple_str = triple_str.into();
        // Hack around this non-conformant triple added in Rust 1.48.
        // https://github.com/EmbarkStudios/cfg-expr/blob/64b460831e020dc108e111663a0a38c922241f9e/tests/eval.rs#L19-L37
        let lexicon_triple = if triple_str == "avr-unknown-gnu-atmega328" {
            target_lexicon::Triple {
                architecture: target_lexicon::Architecture::Avr,
                vendor: target_lexicon::Vendor::Unknown,
                operating_system: target_lexicon::OperatingSystem::Unknown,
                environment: target_lexicon::Environment::Unknown,
                binary_format: target_lexicon::BinaryFormat::Unknown,
            }
        } else {
            match triple_str.parse::<target_lexicon::Triple>() {
                Ok(lexicon_triple) => lexicon_triple,
                Err(lexicon_err) => return Err(TripleParseError::new(triple_str, lexicon_err)),
            }
        };

        Ok(Self {
            triple_str,
            lexicon_triple,
        })
    }

    /// Returns the string corresponding to this triple.
    pub fn as_str(&self) -> &str {
        &self.triple_str
    }

    /// Evaluates this triple against the given platform.
    ///
    /// This simply compares `self` against the `Triple` the platform is based on, ignoring
    /// target features and flags.
    pub fn eval(&self, platform: &Platform) -> bool {
        self == platform.triple()
    }

    // Use cfg-expr's target matcher.
    pub(crate) fn matches(&self, target: &TargetPredicate) -> bool {
        target.matches(&self.lexicon_triple)
    }
}

impl FromStr for Triple {
    type Err = TripleParseError;

    fn from_str(triple_str: &str) -> Result<Self, Self::Err> {
        match triple_str.parse::<target_lexicon::Triple>() {
            Ok(lexicon_triple) => Ok(Self {
                triple_str: triple_str.to_owned().into(),
                lexicon_triple,
            }),
            Err(lexicon_err) => Err(TripleParseError::new(
                triple_str.to_owned().into(),
                lexicon_err,
            )),
        }
    }
}

// ---
// Trait impls
//
// These impls only use the `triple_str`, which is valid because the `lexicon_triple` is a pure
// function of the `triple_str`.
// ---

impl PartialEq for Triple {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.triple_str.eq(&other.triple_str)
    }
}

impl Eq for Triple {}

impl PartialOrd for Triple {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.triple_str.partial_cmp(&other.triple_str)
    }
}

impl Ord for Triple {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.triple_str.cmp(&other.triple_str)
    }
}

impl hash::Hash for Triple {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        hash::Hash::hash(&self.triple_str, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use target_lexicon::*;

    #[test]
    fn test_parse() {
        let target =
            super::Triple::new("x86_64-pc-darwin").expect("this triple is known to target-lexicon");

        let expected_triple = target_lexicon::Triple {
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
