# guppy's architecture

**Authors:** Rain <rain1@fb.com>

**Last review:** 2021-03-08

## guppy: core graph abstractions

`guppy` is built on top of two core abstractions:
* A *package graph*, where the nodes are Cargo packages and the edges are the dependencies between them.
* A *feature graph*, where the nodes are (package, feature) pairs and the edges are the dependencies between them.
  * Every package also has a "base" feature, which is represented as `None` and stands for the package with no features
    enabled.

Both of these graphs use `petgraph`'s `Graph` data structure. This data structure uses integer indexes for nodes and
edges. guppy uses newtype wrappers around `u32`s, `PackageIx` and `FeatureIx`, to ensure that indexes don't accidentally
get mixed up.

### Borrowed data

The main owned types in guppy are `PackageGraph` and `PackageId`. All other types borrow data from a `PackageGraph`, and
the convention throughout this codebase is to use the lifetime parameter `'g` to indicate borrowing from a graph.

Note that rather than returning for example `&'g PackageMetadata`, guppy returns `PackageMetadata<'g>`. This is a
copyable wrapper struct, defined as:

```rust
pub struct PackageMetadata<'g> {
    graph: &'g PackageGraph,
    inner: &'g PackageMetadataImpl,
}
```

These wrapper structs provide more flexibility, so that `PackageMetadata` can have methods on it (like `direct_links`)
which use the graph.

### Building the package graph

TODO

### Building the feature graph

TODO

### Cycles

The package and feature graphs are directed but **not acyclic**: dev-dependencies can introduce cycles. However, many of
the algorithms we care about are designed for acyclic graphs. There are two general ways to handle this:

1. Augment the graph to make it acyclic. For guppy, this could be done by modeling every package using two nodes rather
   than one: non-dev and dev. This would technically make the graph acyclic--but when Rain did a prototype, they found
   that either the implementation's complexity grew out of hand, or the APIs became very unpleasant to use. That is why
   this approach was not pursued further. 

2. Update all the algorithms to also handle cycles. This is the approach that guppy follows.
   
A simple example is to perform depth-first searches using a [postorder traversal], which involves keeping two visit maps
rather than one.

A generic way to adapt DAG algorithms to graphs with cycles is to use [strongly connected components (SCCs)]. An SCC of
a graph is a set of nodes where every node is reachable from every other: it's a formalization of the intuitive notion
of "maximal cycles". One observation is that if every SCC is collapsed down to one node, the resulting graph (called a
**condensation graph**) is acyclic. This suggests a straightforward approach, and the one guppy uses in [`scc.rs`].
1. Build the condensation graph of a package or feature graph.
2. Run the algorithm on the condensation graph.
3. If the resulting set contains any condensed nodes, include all the corresponding source nodes.

But if the overall result set is ordered (e.g. [topologically]), what order should the source nodes of an SCC be added
in? Earlier versions of guppy would add them in arbitrary order, but newer versions add them in *non-dev build order*:
the topological order within the SCC that would arise if all dev-only edges are removed. (Note that this is *not the
same* as an arbitrary topological order over the entire graph with dev-only edges removed: with the
topo-order-within-SCC approach, all the nodes within an SCC are returned together.)

### Cargo build simulations

TODO

## determinator

TODO

## hakari

TODO

[postorder traversal]: https://docs.rs/petgraph/0.5/petgraph/visit/struct.DfsPostOrder.html
[strongly connected components (SCCs)]: https://en.wikipedia.org/wiki/Strongly_connected_component
[`scc.rs`]: https://github.com/facebookincubator/cargo-guppy/blob/c561f51e2b97fd390f6741efbdff26859ffeb769/guppy/src/petgraph_support/scc.rs
[topologically]: https://en.wikipedia.org/wiki/Topological_sorting
