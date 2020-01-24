// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Support for petgraph.
//!
//! The code in here is generic over petgraph's traits, and could be upstreamed into petgraph if
//! desirable.

use petgraph::graph::IndexType;
use petgraph::prelude::*;
use petgraph::visit::{IntoNeighborsDirected, IntoNodeIdentifiers};

pub mod dot;
pub mod reversed;
pub mod walk;

pub fn edge_triple<ER: EdgeRef>(edge_ref: ER) -> (ER::NodeId, ER::NodeId, ER::EdgeId) {
    (edge_ref.source(), edge_ref.target(), edge_ref.id())
}

/// Returns the nodes of a graph that have no incoming edges to them.
pub fn externals<G, Ix>(graph: G) -> impl Iterator<Item = NodeIndex<Ix>>
where
    G: IntoNodeIdentifiers + IntoNeighborsDirected<NodeId = NodeIndex<Ix>>,
    Ix: IndexType,
{
    graph
        .node_identifiers()
        .filter(move |&a| graph.neighbors_directed(a, Incoming).next().is_none())
}
