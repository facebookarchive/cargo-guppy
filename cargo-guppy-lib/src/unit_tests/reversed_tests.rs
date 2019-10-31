//! These tests are for the additional functionality added to ReversedDirected. The base
//! implementation is the same as `petgraph::visit::Reversed` so it is not tested.

use crate::graph::visit::reversed::ReversedDirected;
use petgraph::prelude::*;
use petgraph::visit::{IntoEdges, IntoEdgesDirected};

#[test]
fn reversed_directed_edge_impls() {
    // Directed acyclic graph:
    //
    // A --> B
    // |     |
    // v     v
    // C --> D

    let mut graph = Graph::new();
    let a = graph.add_node("A");
    let b = graph.add_node("B");
    let c = graph.add_node("C");
    let d = graph.add_node("D");
    graph.add_edge(a, b, ());
    graph.add_edge(a, c, ());
    graph.add_edge(b, d, ());
    graph.add_edge(c, d, ());

    // The reversed graph is:
    //
    // A <-- B
    // ^     ^
    // |     |
    // C <-- D
    let reversed = ReversedDirected::new(&graph);

    for source in vec![a, b, c, d] {
        for edge in reversed.edges(source) {
            assert_eq!(edge.source(), source, "edge sources should be correct");
        }
        for edge in reversed.edges_directed(source, Outgoing) {
            assert_eq!(
                edge.source(),
                source,
                "outgoing edge sources should be correct"
            );
        }
        for edge in reversed.edges_directed(source, Incoming) {
            assert_eq!(
                edge.target(),
                source,
                "incoming edge targets should be correct"
            );
        }

        // Check that outgoing edges in the reversed graph are the same as incoming edges in the
        // normal graph, with source and target reversed.
        let mut outgoing: Vec<_> = graph
            .edges_directed(source, Outgoing)
            .map(source_target)
            .collect();
        outgoing.sort();
        let mut reversed_incoming: Vec<_> = reversed
            .edges_directed(source, Incoming)
            .map(target_source)
            .collect();
        reversed_incoming.sort();

        assert_eq!(outgoing, reversed_incoming, "outgoing = reversed incoming");

        // Check that incoming edges in the reversed graph are the same as outgoing edges in the
        // normal graph, with directions reversed.
        let mut incoming: Vec<_> = graph
            .edges_directed(source, Incoming)
            .map(source_target)
            .collect();
        incoming.sort();
        let mut reversed_outgoing: Vec<_> = reversed
            .edges_directed(source, Outgoing)
            .map(target_source)
            .collect();
        reversed_outgoing.sort();
        // `edges` should behave the same way as `edges_directed` with `Outgoing`.
        let mut reversed_edges: Vec<_> = reversed.edges(source).map(target_source).collect();
        reversed_edges.sort();

        assert_eq!(incoming, reversed_outgoing, "incoming = reversed outgoing");
        assert_eq!(incoming, reversed_edges, "incoming = reversed edges");
    }
}

fn source_target<ER: EdgeRef>(edge: ER) -> (ER::NodeId, ER::NodeId) {
    (edge.source(), edge.target())
}

fn target_source<ER: EdgeRef>(edge: ER) -> (ER::NodeId, ER::NodeId) {
    (edge.target(), edge.source())
}
