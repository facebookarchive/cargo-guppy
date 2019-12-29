// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use petgraph::graph;
use petgraph::prelude::*;
use petgraph::visit::{NodeFiltered, Reversed, ReversedEdgeReference};

// ---
// New traits
// ---

/// Provides a way to flip source and target indexes for reversed graphs.
///
/// Some operations that are generic over forward and reverse graphs may want the original
/// direction. This trait provides that.
///
/// For convenience, this is implemented for both graphs and `EdgeRef` types.
pub trait ReverseFlip {
    /// Whether this graph is reversed.
    fn is_reversed() -> bool;

    /// Flip the source and target indexes if this is a reversed graph. Leave them the same if it
    /// isn't.
    fn reverse_flip<N>(source: N, target: N) -> (N, N) {
        if Self::is_reversed() {
            (target, source)
        } else {
            (source, target)
        }
    }
}

// TODO: implement ReverseFlip for all the other base graph types as well.

impl<'a, NW, EW, Ty, Ix> ReverseFlip for &'a Graph<NW, EW, Ty, Ix> {
    fn is_reversed() -> bool {
        false
    }
}

impl<G: ReverseFlip> ReverseFlip for Reversed<G> {
    fn is_reversed() -> bool {
        !G::is_reversed()
    }
}

impl<G, F> ReverseFlip for NodeFiltered<G, F>
where
    G: ReverseFlip,
{
    fn is_reversed() -> bool {
        G::is_reversed()
    }
}

impl<'a, E, Ix> ReverseFlip for graph::EdgeReference<'a, E, Ix> {
    fn is_reversed() -> bool {
        false
    }
}

impl<ER> ReverseFlip for ReversedEdgeReference<ER>
where
    ER: ReverseFlip,
{
    fn is_reversed() -> bool {
        !ER::is_reversed()
    }
}

impl<'a, T: ReverseFlip> ReverseFlip for &'a T {
    fn is_reversed() -> bool {
        T::is_reversed()
    }
}
