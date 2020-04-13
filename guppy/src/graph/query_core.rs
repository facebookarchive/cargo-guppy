// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::GraphSpec;
use fixedbitset::FixedBitSet;
use petgraph::graph::IndexType;
use petgraph::prelude::*;
use petgraph::visit::{IntoNeighbors, Visitable};

#[derive(Clone, Debug)]
pub(super) enum QueryParams<G: GraphSpec> {
    Forward(Vec<NodeIndex<G::Ix>>),
    Reverse(Vec<NodeIndex<G::Ix>>),
}

pub(super) fn all_visit_map<G, Ix>(graph: G) -> (FixedBitSet, usize)
where
    G: Visitable<NodeId = NodeIndex<Ix>, Map = FixedBitSet>,
    Ix: IndexType,
{
    let mut visit_map = graph.visit_map();
    // Mark all nodes visited.
    visit_map.insert_range(..);
    let len = visit_map.len();
    (visit_map, len)
}

pub(super) fn reachable_map<G, Ix>(graph: G, roots: Vec<G::NodeId>) -> (FixedBitSet, usize)
where
    G: Visitable<NodeId = NodeIndex<Ix>, Map = FixedBitSet> + IntoNeighbors,
    Ix: IndexType,
{
    // To figure out what nodes are reachable, run a DFS starting from the roots.
    // This is DfsPostOrder since that handles cycles while a regular DFS doesn't.
    let mut dfs = DfsPostOrder::empty(graph);
    dfs.stack = roots;
    while let Some(_) = dfs.next(graph) {}

    // Once the DFS is done, the discovered map (or the finished map) is what's reachable.
    debug_assert_eq!(
        dfs.discovered, dfs.finished,
        "discovered and finished maps match at the end"
    );
    let reachable = dfs.discovered;
    let len = reachable.count_ones(..);
    (reachable, len)
}
