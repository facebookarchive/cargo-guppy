// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::edge_triple;
use petgraph::visit::{IntoEdges, VisitMap, Visitable, Walker};
use std::iter;

#[derive(Clone, Debug)]
pub(crate) struct EdgeDfs<E, N, VM> {
    /// The queue of (source, target, edge) to visit.
    pub stack: Vec<(N, N, E)>,
    /// The map of discovered nodes
    pub discovered: VM,
}

impl<E, N, VM> EdgeDfs<E, N, VM>
where
    E: Copy + PartialEq,
    N: Copy + PartialEq,
    VM: VisitMap<N>,
{
    /// Creates a new EdgeBfs, using the graph's visitor map, and puts all edges out of `initials`
    /// in the queue of edges to visit.
    pub(crate) fn new<G>(graph: G, initials: impl IntoIterator<Item = N>) -> Self
    where
        G: Visitable<Map = VM> + IntoEdges<NodeId = N, EdgeId = E>,
    {
        let mut discovered = graph.visit_map();
        let stack = initials
            .into_iter()
            .flat_map(|node_idx| {
                discovered.visit(node_idx);
                graph.edges(node_idx).map(edge_triple)
            })
            .collect();
        Self { stack, discovered }
    }

    /// Creates a new EdgeBfs, using the graph's visitor map, and puts all edges out of `start`
    /// in the queue of edges to visit.
    #[allow(dead_code)]
    pub(crate) fn new_single<G>(graph: G, start: N) -> Self
    where
        G: Visitable<Map = VM> + IntoEdges<NodeId = N, EdgeId = E>,
    {
        Self::new(graph, iter::once(start))
    }

    /// Return the next edge in the bfs, or `None` if no more edges remain.
    pub fn next<G>(&mut self, graph: G) -> Option<(N, N, E)>
    where
        G: IntoEdges<NodeId = N, EdgeId = E>,
    {
        self.stack.pop().map(|(source, target, edge)| {
            if self.discovered.visit(target) {
                self.stack.extend(graph.edges(target).map(edge_triple));
            }
            (source, target, edge)
        })
    }
}

impl<G> Walker<G> for EdgeDfs<G::EdgeId, G::NodeId, G::Map>
where
    G: IntoEdges + Visitable,
{
    type Item = (G::NodeId, G::NodeId, G::EdgeId);

    fn walk_next(&mut self, context: G) -> Option<Self::Item> {
        self.next(context)
    }
}
