use petgraph::visit::{EdgeRef, IntoEdges, VisitMap, Visitable, Walker};
use std::collections::VecDeque;
use std::iter;

#[derive(Clone, Debug)]
pub(crate) struct EdgeBfs<E, N, VM> {
    /// The queue of (source, target, edge) to visit.
    pub queue: VecDeque<(N, N, E)>,
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
            graph.edges(start).map(edge_triple)
        }));
        Self { queue, discovered }
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
        self.queue.pop_front().map(|(source, target, edge)| {
            if self.discovered.visit(target) {
                self.queue.extend(graph.edges(target).map(edge_triple));
            }
            (source, target, edge)
        })
    }
}

impl<G> Walker<G> for EdgeBfs<G::EdgeId, G::NodeId, G::Map>
where
    G: IntoEdges + Visitable,
{
    type Item = (G::NodeId, G::NodeId, G::EdgeId);

    fn walk_next(&mut self, context: G) -> Option<Self::Item> {
        self.next(context)
    }
}

fn edge_triple<ER: EdgeRef>(edge_ref: ER) -> (ER::NodeId, ER::NodeId, ER::EdgeId) {
    (edge_ref.source(), edge_ref.target(), edge_ref.id())
}
