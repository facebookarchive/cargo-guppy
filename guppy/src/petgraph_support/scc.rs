// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use fixedbitset::FixedBitSet;
use petgraph::algo::kosaraju_scc;
use petgraph::graph::IndexType;
use petgraph::prelude::*;
use petgraph::visit::{IntoNeighborsDirected, IntoNodeIdentifiers, VisitMap, Visitable};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub(crate) struct Sccs<Ix: IndexType> {
    // TODO: This is somewhat inefficient as storage -- might be better to use something like Nested
    // once https://github.com/tafia/nested/pull/1 lands.
    sccs: Vec<Vec<NodeIndex<Ix>>>,
    multi_map: HashMap<NodeIndex<Ix>, usize>,
}

impl<Ix: IndexType> Sccs<Ix> {
    /// Creates a new instance from the provided graph.
    pub fn new<G>(graph: G) -> Self
    where
        G: IntoNeighborsDirected<NodeId = NodeIndex<Ix>> + Visitable + IntoNodeIdentifiers,
        <G as Visitable>::Map: VisitMap<NodeIndex<Ix>>,
    {
        // Use kosaraju_scc since it is iterative (tarjan_scc is recursive) and package graphs
        // have unbounded depth.
        let sccs = kosaraju_scc(graph);
        let mut multi_map = HashMap::new();
        for (idx, scc) in sccs.iter().enumerate() {
            if scc.len() > 1 {
                multi_map.extend(scc.iter().map(|ix| (*ix, idx)));
            }
        }
        Self { sccs, multi_map }
    }

    /// Returns true if `a` and `b` are in the same scc.
    pub fn is_same_scc(&self, a: NodeIndex<Ix>, b: NodeIndex<Ix>) -> bool {
        if a == b {
            return true;
        }
        match (self.multi_map.get(&a), self.multi_map.get(&b)) {
            (Some(a_scc), Some(b_scc)) => a_scc == b_scc,
            _ => false,
        }
    }

    /// Returns all the SCCs in this graph.
    pub fn sccs(&self) -> &[Vec<NodeIndex<Ix>>] {
        &self.sccs
    }

    /// Returns all the SCCs with more than one element.
    pub fn multi_sccs(&self) -> impl Iterator<Item = &[NodeIndex<Ix>]> {
        self.sccs.iter().filter_map(|scc| {
            if scc.len() > 1 {
                Some(scc.as_slice())
            } else {
                None
            }
        })
    }

    /// Returns all the nodes of this graph that have no incoming edges to them, and all the nodes
    /// in an SCC into which there are no incoming edges.
    pub fn externals<'a, G>(&'a self, graph: G) -> impl Iterator<Item = NodeIndex<Ix>> + 'a
    where
        G: 'a + IntoNodeIdentifiers + IntoNeighborsDirected<NodeId = NodeIndex<Ix>>,
        Ix: IndexType,
    {
        // Consider each SCC as one logical node.
        let mut external_sccs = FixedBitSet::with_capacity(self.sccs.len());
        let mut internal_sccs = FixedBitSet::with_capacity(self.sccs.len());
        graph
            .node_identifiers()
            .filter(move |ix| match self.multi_map.get(ix) {
                Some(&scc_idx) => {
                    // Consider one node identifier for each scc -- whichever one comes first.
                    if external_sccs.contains(scc_idx) {
                        return true;
                    }
                    if internal_sccs.contains(scc_idx) {
                        return false;
                    }

                    let scc = &self.sccs[scc_idx];
                    let is_external = scc
                        .iter()
                        .flat_map(|ix| {
                            // Look at all incoming nodes from every SCC member.
                            graph.neighbors_directed(*ix, Incoming)
                        })
                        .all(|neighbor_ix| {
                            // * Accept any nodes are in the same SCC.
                            // * Any other results imply that this isn't an external scc.
                            match self.multi_map.get(&neighbor_ix) {
                                Some(neighbor_scc_idx) => neighbor_scc_idx == &scc_idx,
                                None => false,
                            }
                        });
                    if is_external {
                        external_sccs.insert(scc_idx);
                    } else {
                        internal_sccs.insert(scc_idx);
                    }
                    is_external
                }
                None => {
                    // Not part of an SCC -- just look at whether there are any incoming nodes
                    // at all.
                    graph.neighbors_directed(*ix, Incoming).next().is_none()
                }
            })
    }

    /// Iterate over all nodes in the direction specified.
    pub fn node_iter(&self, direction: Direction) -> NodeIter<Ix> {
        NodeIter {
            sccs: self.sccs(),
            direction,
            next_scc: 0,
            next_idx: 0,
        }
    }
}

/// An iterator over the nodes of strongly connected components.
#[derive(Clone, Debug)]
pub(crate) struct NodeIter<'a, Ix> {
    sccs: &'a [Vec<NodeIndex<Ix>>],
    direction: Direction,
    next_scc: usize,
    next_idx: usize,
}

impl<'a, Ix> NodeIter<'a, Ix> {
    /// Returns the direction this iteration is happening in.
    pub fn direction(&self) -> Direction {
        self.direction
    }
}

impl<'a, Ix: IndexType> Iterator for NodeIter<'a, Ix> {
    type Item = NodeIndex<Ix>;

    fn next(&mut self) -> Option<NodeIndex<Ix>> {
        // note that outgoing implies iterating over the sccs in reverse order, while incoming means
        // sccs in forward order
        // This would be easy to do using flat_map, but then the type can't be named :(
        // It would also be easy to do using https://github.com/tafia/nested once available!
        if self.direction == Direction::Outgoing {
            loop {
                let prev_scc_plus_1 = self.sccs.len() - self.next_scc;
                if prev_scc_plus_1 == 0 {
                    return None;
                }
                // This won't panic because prev_scc_plus_1 >= 1.
                let prev_scc = &self.sccs[prev_scc_plus_1 - 1];

                let prev_idx_plus_1 = prev_scc.len() - self.next_idx;
                if prev_idx_plus_1 == 0 {
                    // Exhausted this SCC -- move to the next one.
                    self.next_scc += 1;
                    self.next_idx = 0;
                    continue;
                }

                self.next_idx += 1;
                // This won't panic because prev_idx_plus_1 >= 1.
                let ix = &prev_scc[prev_idx_plus_1 - 1];
                return Some(*ix);
            }
        } else {
            // This looks different but is basically the same as the loop above.
            while let Some(next_scc) = self.sccs.get(self.next_scc) {
                match next_scc.get(self.next_idx) {
                    Some(ix) => {
                        self.next_idx += 1;
                        return Some(*ix);
                    }
                    None => {
                        // Exhausted this SCC -- move to the next one.
                        self.next_scc += 1;
                        self.next_idx = 0;
                    }
                }
            }
            // Exhausted all SCCs.
            None
        }
    }
}
