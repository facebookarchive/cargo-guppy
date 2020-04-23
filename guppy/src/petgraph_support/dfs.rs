// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use petgraph::prelude::*;
use petgraph::visit::{DfsPostOrder, IntoEdges, VisitMap};

/// `DfsPostOrder::next`, adapted for a filter that's a `FnMut`.
pub fn dfs_next_filtered<N, VM, G>(
    dfs: &mut DfsPostOrder<N, VM>,
    graph: G,
    mut edge_filter: impl FnMut(G::EdgeRef) -> bool,
) -> Option<N>
where
    N: Copy + PartialEq,
    VM: VisitMap<N>,
    G: IntoEdges<NodeId = N>,
{
    // Adapted from DfsPostOrder::next in petgraph 0.5.0.
    while let Some(&nx) = dfs.stack.last() {
        if dfs.discovered.visit(nx) {
            // First time visiting `nx`: Push neighbors, don't pop `nx`
            let neighbors = graph.edges(nx).filter_map(|edge| {
                if edge_filter(edge) {
                    Some(edge.target())
                } else {
                    None
                }
            });
            for succ in neighbors {
                if !dfs.discovered.is_visited(&succ) {
                    dfs.stack.push(succ);
                }
            }
        } else {
            dfs.stack.pop();
            if dfs.finished.visit(nx) {
                // Second time: All reachable nodes must have been finished
                return Some(nx);
            }
        }
    }
    None
}
