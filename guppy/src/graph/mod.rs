// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use cargo_metadata::DependencyKind;
use petgraph::prelude::*;

mod build;
mod graph;
mod print;
mod select;

// Public exports for dot graphs.
pub use crate::petgraph_support::dot::DotWrite;
pub use graph::*;
pub use print::PackageDotVisitor;
pub use select::{DependencyLinkIter, PackageIdIter, PackageSelect};

/// The direction in which to follow dependencies.
///
/// Used by the `_directed` methods.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DependencyDirection {
    /// Dependencies from this package to other packages.
    Forward,
    /// Reverse dependencies from other packages to this one.
    Reverse,
}

impl DependencyDirection {
    /// Returns the opposite direction to this one.
    pub fn opposite(&self) -> Self {
        match self {
            DependencyDirection::Forward => DependencyDirection::Reverse,
            DependencyDirection::Reverse => DependencyDirection::Forward,
        }
    }

    fn to_direction(self) -> Direction {
        match self {
            DependencyDirection::Forward => Direction::Outgoing,
            DependencyDirection::Reverse => Direction::Incoming,
        }
    }
}

fn kind_str(kind: DependencyKind) -> &'static str {
    match kind {
        DependencyKind::Normal => "normal",
        DependencyKind::Build => "build",
        DependencyKind::Development => "dev",
        _ => "unknown",
    }
}
