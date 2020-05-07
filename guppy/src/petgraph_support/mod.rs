// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Support for petgraph.
//!
//! The code in here is generic over petgraph's traits, and could be upstreamed into petgraph if
//! desirable.

use fixedbitset::FixedBitSet;
use petgraph::graph::IndexType;
use petgraph::prelude::*;
use std::iter::FromIterator;

pub mod dfs;
pub mod dot;
pub mod reversed;
pub mod scc;
pub mod walk;

pub fn edge_triple<ER: EdgeRef>(edge_ref: ER) -> (ER::NodeId, ER::NodeId, ER::EdgeId) {
    (edge_ref.source(), edge_ref.target(), edge_ref.id())
}

pub struct IxBitSet(pub FixedBitSet);

impl From<IxBitSet> for FixedBitSet {
    fn from(ix_set: IxBitSet) -> Self {
        ix_set.0
    }
}

impl<Ix: IndexType> FromIterator<NodeIndex<Ix>> for IxBitSet {
    fn from_iter<T: IntoIterator<Item = NodeIndex<Ix>>>(iter: T) -> Self {
        IxBitSet(iter.into_iter().map(|node_ix| node_ix.index()).collect())
    }
}

impl<Ix: IndexType> FromIterator<EdgeIndex<Ix>> for IxBitSet {
    fn from_iter<T: IntoIterator<Item = EdgeIndex<Ix>>>(iter: T) -> Self {
        IxBitSet(iter.into_iter().map(|edge_ix| edge_ix.index()).collect())
    }
}
