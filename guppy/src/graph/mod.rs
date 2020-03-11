// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Entry point for analyzing Cargo dependency graphs.
//!
//! The main entry point for analyzing graphs is [`PackageGraph`](struct.PackageGraph.html). See its
//! documentation for more details.

use cargo_metadata::{DependencyKind, PackageId};
use petgraph::prelude::*;
use std::fmt;

mod build;
#[doc(hidden)]
pub mod feature;
mod graph_impl;
mod print;
#[cfg(feature = "proptest09")]
mod proptest09;
mod select;

pub use crate::petgraph_support::dot::DotWrite;
pub use graph_impl::*;
use once_cell::sync::Lazy;
use petgraph::graph::IndexType;
pub use print::*;
pub use select::*;
use semver::{Version, VersionReq};

/// The direction in which to follow dependencies.
///
/// Used by the `_directed` methods.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "proptest09", derive(proptest_derive::Arbitrary))]
pub enum DependencyDirection {
    /// Dependencies from this package to other packages.
    Forward,
    /// Reverse dependencies from other packages to this one.
    Reverse,
}

impl DependencyDirection {
    /// Returns the opposite direction to this one.
    pub fn opposite(self) -> Self {
        match self {
            DependencyDirection::Forward => DependencyDirection::Reverse,
            DependencyDirection::Reverse => DependencyDirection::Forward,
        }
    }
}

impl From<Direction> for DependencyDirection {
    fn from(direction: Direction) -> Self {
        match direction {
            Direction::Outgoing => DependencyDirection::Forward,
            Direction::Incoming => DependencyDirection::Reverse,
        }
    }
}

impl From<DependencyDirection> for Direction {
    fn from(direction: DependencyDirection) -> Self {
        match direction {
            DependencyDirection::Forward => Direction::Outgoing,
            DependencyDirection::Reverse => Direction::Incoming,
        }
    }
}

/// Index for PackageGraph. Used for newtype wrapping.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct PackageIx(u32);

/// Index for FeatureGraph. Used for newtype wrapping.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct FeatureIx(u32);

macro_rules! graph_ix {
    ($ix_type: ident) => {
        impl fmt::Display for $ix_type {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        // From the docs for `IndexType`:
        //
        // > Marked `unsafe` because: the trait must faithfully preseve and convert index values.
        unsafe impl IndexType for $ix_type {
            #[inline(always)]
            fn new(x: usize) -> Self {
                $ix_type(x as u32)
            }
            #[inline(always)]
            fn index(&self) -> usize {
                self.0 as usize
            }
            #[inline(always)]
            fn max() -> Self {
                $ix_type(::std::u32::MAX)
            }
        }
    };
}

graph_ix!(PackageIx);
graph_ix!(FeatureIx);

/// Used to group together associated types with a particular graph.
trait GraphSpec {
    type Node;
    type Edge;
    type Ix: IndexType;
}

impl GraphSpec for PackageGraph {
    type Node = PackageId;
    type Edge = DependencyEdge;
    type Ix = PackageIx;
}

impl<'g> GraphSpec for feature::FeatureGraph<'g> {
    type Node = feature::FeatureNode;
    type Edge = feature::FeatureEdge;
    type Ix = FeatureIx;
}

fn kind_str(kind: DependencyKind) -> &'static str {
    match kind {
        DependencyKind::Normal => "normal",
        DependencyKind::Build => "build",
        DependencyKind::Development => "dev",
        _ => "unknown",
    }
}

// A requirement of "*" filters out pre-release versions with the semver crate,
// but cargo accepts them.
// See https://github.com/steveklabnik/semver/issues/98.
fn cargo_version_matches(req: &VersionReq, version: &Version) -> bool {
    static MAJOR_WILDCARD: Lazy<VersionReq> = Lazy::new(|| VersionReq::parse("*").unwrap());

    req == &*MAJOR_WILDCARD || req.matches(version)
}
