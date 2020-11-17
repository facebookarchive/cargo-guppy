// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Error types returned by the determinator.

use crate::rules::RuleIndex;
use std::{error, fmt};

/// An error that occurred while resolving a set of determinator rules.
#[derive(Debug)]
pub struct RulesError {
    rule_index: RuleIndex,
    kind: RulesErrorKind,
}

impl RulesError {
    /// Returns the index of the determinator rule that failed to parse.
    pub fn rule_index(&self) -> RuleIndex {
        self.rule_index
    }

    /// Returns the kind of error that occurred.
    pub fn kind(&self) -> &RulesErrorKind {
        &self.kind
    }

    // ---
    // Internal constructors
    // ---

    pub(crate) fn resolve_ref(rule_index: RuleIndex, err: guppy::Error) -> Self {
        Self {
            rule_index,
            kind: RulesErrorKind::ResolveRef(err),
        }
    }

    pub(crate) fn glob_parse(rule_index: RuleIndex, err: globset::Error) -> Self {
        let kind = RulesErrorKind::GlobParse {
            glob: err.glob().map(|s| s.to_owned()),
            err: Box::new(err),
        };
        Self { rule_index, kind }
    }
}

impl fmt::Display for RulesError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "error while resolving determinator rules: {}: {}",
            self.rule_index, self.kind
        )
    }
}

impl error::Error for RulesError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self.kind {
            RulesErrorKind::ResolveRef(err) => Some(err),
            RulesErrorKind::GlobParse { err, .. } => Some(&**err),
        }
    }
}

/// The kind of error that occurred while parsing a set of determinator rules.
#[derive(Debug)]
#[non_exhaustive]
pub enum RulesErrorKind {
    /// An error occurred while resolving a reference in guppy.
    ResolveRef(guppy::Error),

    /// An error occurred while parsing a glob.
    GlobParse {
        /// The glob that failed to parse, if one was present.
        glob: Option<String>,
        /// The error that occurred while parsing the glob.
        err: Box<dyn error::Error + Send + Sync>,
    },
}

impl fmt::Display for RulesErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RulesErrorKind::ResolveRef(err) => write!(f, "{}", err),
            RulesErrorKind::GlobParse {
                glob: Some(glob),
                err,
            } => write!(f, "while parsing glob '{}': {}", glob, err),
            RulesErrorKind::GlobParse { glob: None, err } => {
                write!(f, "while parsing a glob: {}", err)
            }
        }
    }
}
