// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::{DependencyDirection, GraphSpec};
use crate::petgraph_support::scc::Sccs;
use fixedbitset::FixedBitSet;
use petgraph::graph::IndexType;
use petgraph::prelude::*;
use petgraph::visit::{IntoNeighbors, NodeFiltered, Reversed, Visitable};

#[derive(Clone, Debug)]
pub(super) enum SelectParams<G: GraphSpec> {
    All,
    SelectForward(Vec<NodeIndex<G::Ix>>),
    SelectReverse(Vec<NodeIndex<G::Ix>>),
}

impl<G: GraphSpec> SelectParams<G> {
    pub(super) fn default_direction(&self) -> DependencyDirection {
        match self {
            SelectParams::All | SelectParams::SelectForward(_) => DependencyDirection::Forward,
            SelectParams::SelectReverse(_) => DependencyDirection::Reverse,
        }
    }
}

/// Computes intermediate state for operations where the graph can be filtered dynamically if
/// possible.
///
/// Note that the second return value is the initial starting points of a graph traversal, which
/// might be a superset of the actual roots.
pub(super) fn select_postfilter<G: GraphSpec>(
    graph: &Graph<G::Node, G::Edge, Directed, G::Ix>,
    params: SelectParams<G>,
    sccs: &Sccs<G::Ix>,
    direction: DependencyDirection,
) -> (Option<FixedBitSet>, Vec<NodeIndex<G::Ix>>) {
    use DependencyDirection::*;
    use SelectParams::*;

    // If any element of an SCC is in the reachable map, so would every other element. This means
    // that any SCC map computed on the full graph will work on a prefiltered graph. (This will
    // change if we decide to implement edge visiting/filtering.)
    match (params, direction) {
        (All, Forward) => {
            // No need for a reachable map, and use all roots.
            let roots: Vec<_> = sccs.externals(graph).collect();
            (None, roots)
        }
        (All, Reverse) => {
            // No need for a reachable map, and use all roots.
            let reversed_graph = Reversed(graph);
            let roots: Vec<_> = sccs.externals(reversed_graph).collect();
            (None, roots)
        }
        (SelectForward(initials), Forward) => {
            // No need for a reachable map.
            (None, initials)
        }
        (SelectForward(initials), Reverse) => {
            // Forward traversal + reverse order = need to compute reachable map.
            let (reachable, _) = reachable_map(graph, initials);
            let filtered_reversed_graph = NodeFiltered(Reversed(graph), reachable);
            // The filtered + reversed graph will have its own roots since the iteration order
            // is reversed from the specified roots.
            let roots: Vec<_> = sccs.externals(&filtered_reversed_graph).collect();

            (Some(filtered_reversed_graph.1), roots)
        }
        (SelectReverse(initials), Forward) => {
            // Reverse traversal + forward order = need to compute reachable map.
            let reversed_graph = Reversed(graph);
            let (reachable, _) = reachable_map(reversed_graph, initials);
            let filtered_graph = NodeFiltered(graph, reachable);
            // The filtered graph will have its own roots since the iteration order is reversed
            // from the specified roots.
            let roots: Vec<_> = sccs.externals(&filtered_graph).collect();

            (Some(filtered_graph.1), roots)
        }
        (SelectReverse(initials), Reverse) => {
            // No need for a reachable map.
            (None, initials)
        }
    }
}

pub(super) fn all_visit_map<G, Ix>(graph: G) -> (FixedBitSet, usize)
where
    G: Visitable<NodeId = NodeIndex<Ix>, Map = FixedBitSet>,
    Ix: IndexType,
{
    let mut visit_map = graph.visit_map();
    // Mark all nodes visited.
    visit_map.insert_range(..);
    let count = visit_map.len();
    (visit_map, count)
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
    let count = reachable.count_ones(..);
    (reachable, count)
}
