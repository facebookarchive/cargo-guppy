// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use petgraph::graph::{EdgeReference, IndexType};
use petgraph::prelude::*;
use petgraph::visit::ReversedEdgeReference;

// ---
// New traits
// ---

/// Provides a way to flip source and target indexes for reversed graphs.
///
/// Some operations that are generic over forward and reverse graphs may want the original
/// direction. This trait provides that.
///
/// For convenience, this is implemented for both graphs and `EdgeRef` types.
pub trait MaybeReversedEdge: EdgeRef {
    /// Whether this edge reference is reversed.
    fn is_reversed() -> bool;

    /// Returns the original source, right side up.
    fn original_source(&self) -> Self::NodeId {
        if Self::is_reversed() {
            self.target()
        } else {
            self.source()
        }
    }

    /// Returns the original target, right side up.
    fn original_target(&self) -> Self::NodeId {
        if Self::is_reversed() {
            self.source()
        } else {
            self.target()
        }
    }

    /// Returns the original (source, target), right side up.
    fn original_endpoints(&self) -> (Self::NodeId, Self::NodeId) {
        if Self::is_reversed() {
            (self.target(), self.source())
        } else {
            (self.source(), self.target())
        }
    }
}

impl<'a, E, Ix: IndexType> MaybeReversedEdge for EdgeReference<'a, E, Ix> {
    fn is_reversed() -> bool {
        false
    }
}

impl<ER> MaybeReversedEdge for ReversedEdgeReference<ER>
where
    ER: MaybeReversedEdge,
{
    fn is_reversed() -> bool {
        !ER::is_reversed()
    }
}
