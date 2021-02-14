// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use petgraph::{
    graph::IndexType,
    prelude::*,
    visit::{
        GraphRef, IntoNeighborsDirected, IntoNodeIdentifiers, NodeCompactIndexable, Visitable,
        Walker,
    },
};

/// A cycle-aware topological sort of a graph.
#[derive(Clone, Debug)]
pub struct TopoWithCycles<Ix> {
    topo: Box<[NodeIndex<Ix>]>,
    reverse_index: Box<[usize]>,
}

impl<Ix: IndexType> TopoWithCycles<Ix> {
    pub fn new<G>(graph: G) -> Self
    where
        G: GraphRef
            + Visitable<NodeId = NodeIndex<Ix>>
            + IntoNodeIdentifiers
            + IntoNeighborsDirected<NodeId = NodeIndex<Ix>>
            + NodeCompactIndexable,
    {
        // petgraph's default topo algorithms don't handle cycles. Use DfsPostOrder which does.
        let mut dfs = DfsPostOrder::empty(graph);
        dfs.stack.extend(
            graph
                .node_identifiers()
                .filter(move |&a| graph.neighbors_directed(a, Incoming).next().is_none()),
        );
        let mut topo: Vec<NodeIndex<Ix>> = dfs.iter(graph).collect();
        // dfs returns its data in postorder (reverse topo order), so reverse that for forward topo
        // order.
        topo.reverse();

        // Because the graph is NodeCompactIndexable, the indexes are in the range (0..topo.len()).
        // Use this property to build a reverse map.
        let mut reverse_index = vec![0; topo.len()];
        topo.iter().enumerate().for_each(|(topo_ix, node_ix)| {
            reverse_index[node_ix.index()] = topo_ix;
        });

        Self {
            topo: topo.into_boxed_slice(),
            reverse_index: reverse_index.into_boxed_slice(),
        }
    }

    /// Sort nodes based on the topo order in self.
    #[inline]
    pub fn sort_nodes(&self, nodes: &mut Vec<NodeIndex<Ix>>) {
        nodes.sort_unstable_by_key(|node_ix| self.topo_ix(*node_ix))
    }

    #[inline]
    pub fn topo_ix(&self, node_ix: NodeIndex<Ix>) -> usize {
        self.reverse_index[node_ix.index()]
    }
}
