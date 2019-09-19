use petgraph::visit::{EdgeRef, IntoEdges, VisitMap, Visitable};
use std::collections::VecDeque;
use std::iter;

pub(crate) struct EdgeBfs<E, N, VM> {
    /// The queue of edges to visit, along with their target nodes.
    pub queue: VecDeque<(E, N)>,
    /// The map of discovered nodes
    pub discovered: VM,
}

impl<E, N, VM> EdgeBfs<E, N, VM>
where
    E: Copy + PartialEq,
    N: Copy + PartialEq,
    VM: VisitMap<N>,
{
    /// Creates a new EdgeBfs, using the graph's visitor map, and puts all edges out of `starts`
    /// in the queue of edges to visit.
    pub(crate) fn new<G>(graph: G, starts: impl IntoIterator<Item = N>) -> Self
    where
        G: Visitable<Map = VM> + IntoEdges<NodeId = N, EdgeId = E>,
    {
        let mut discovered = graph.visit_map();
        let mut queue = VecDeque::new();
        queue.extend(starts.into_iter().flat_map(|start| {
            discovered.visit(start);
            graph.edges(start).map(edge_id_and_target)
        }));
        Self { queue, discovered }
    }

    /// Creates a new EdgeBfs, using the graph's visitor map, and puts all edges out of `start`
    /// in the queue of edges to visit.
    pub(crate) fn new_single<G>(graph: G, start: N) -> Self
    where
        G: Visitable<Map = VM> + IntoEdges<NodeId = N, EdgeId = E>,
    {
        Self::new(graph, iter::once(start))
    }

    /// Return the next edge in the bfs, or `None` if no more edges remain.
    pub fn next<G>(&mut self, graph: G) -> Option<E>
    where
        G: IntoEdges<NodeId = N, EdgeId = E>,
    {
        self.queue.pop_front().map(|(edge, target)| {
            if self.discovered.visit(target) {
                self.queue
                    .extend(graph.edges(target).map(edge_id_and_target));
            }
            edge
        })
    }
}

fn edge_id_and_target<ER: EdgeRef>(edge_ref: ER) -> (ER::EdgeId, ER::NodeId) {
    (edge_ref.id(), edge_ref.target())
}
