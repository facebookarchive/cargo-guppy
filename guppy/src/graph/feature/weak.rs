// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Support for weak features.

use crate::graph::PackageIx;
use indexmap::IndexSet;
use petgraph::graph::NodeIndex;

/// Data structure that tracks pairs of package indexes that form weak dependencies.
#[derive(Debug)]
pub(super) struct WeakDependencies {
    ixs: IndexSet<(NodeIndex<PackageIx>, NodeIndex<PackageIx>)>,
}

impl WeakDependencies {
    pub(super) fn new() -> Self {
        Self {
            ixs: IndexSet::new(),
        }
    }

    pub(super) fn insert(
        &mut self,
        from_ix: NodeIndex<PackageIx>,
        to_ix: NodeIndex<PackageIx>,
    ) -> WeakIndex {
        WeakIndex(self.ixs.insert_full((from_ix, to_ix)).0)
    }
}

// Not part of the public API -- exposed for testing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[doc(hidden)]
pub struct WeakIndex(pub(super) usize);
