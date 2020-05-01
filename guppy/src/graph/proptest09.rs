// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::{PackageGraph, PackageLink, PackageQuery, PackageResolver, Workspace};
use crate::PackageId;
use fixedbitset::FixedBitSet;
use petgraph::prelude::*;
use petgraph::visit::VisitMap;
use proptest::collection::vec;
use proptest::prelude::*;

/// ## Helpers for property testing
///
/// The methods in this section allow a `PackageGraph` to be used in property-based testing
/// scenarios.
///
/// Currently, [proptest 0.9](https://docs.rs/proptest/0.9) is supported if the `proptest09` feature
/// is enabled.
impl PackageGraph {
    /// Returns a `Strategy` that generates random package IDs from this graph.
    ///
    /// Requires the `proptest09` feature to be enabled.
    ///
    /// ## Panics
    ///
    /// Panics if there are no packages in this `PackageGraph`.
    pub fn prop09_id_strategy<'g>(&'g self) -> impl Strategy<Value = &'g PackageId> + 'g {
        let dep_graph = &self.dep_graph;
        any::<prop::sample::Index>().prop_map(move |index| {
            let package_ix = NodeIndex::new(index.index(dep_graph.node_count()));
            &self.dep_graph[package_ix]
        })
    }

    /// Returns a `Strategy` that generates random dependency links from this graph.
    ///
    /// Requires the `proptest09` feature to be enabled.
    ///
    /// ## Panics
    ///
    /// Panics if there are no dependency edges in this `PackageGraph`.
    pub fn prop09_link_strategy<'g>(&'g self) -> impl Strategy<Value = PackageLink<'g>> + 'g {
        any::<prop::sample::Index>().prop_map(move |index| {
            // Note that this works because PackageGraph uses petgraph::Graph, not StableGraph. If
            // PackageGraph used StableGraph, a retain_edges call would create holes -- invalid
            // indexes in the middle of the range. Graph compacts edge indexes so that all
            // indexes from 0 to link_count are valid.
            let edge_ix = EdgeIndex::new(index.index(self.link_count()));
            let (source_ix, target_ix) = self
                .dep_graph
                .edge_endpoints(edge_ix)
                .expect("all edge indexes 0..link_count should be valid");
            self.edge_to_link(source_ix, target_ix, edge_ix, None)
        })
    }

    /// Returns a `Strategy` that generates a random `PackageResolver` instance from this graph.
    ///
    /// Requires the `proptest09` feature to be enabled.
    pub fn prop09_resolver_strategy<'g>(&'g self) -> impl Strategy<Value = Prop09Resolver> + 'g {
        // Generate a FixedBitSet to filter based off of.
        fixedbitset_strategy(self.dep_graph.edge_count()).prop_map(Prop09Resolver::new)
    }
}

/// ## Helpers for property testing
///
/// The methods in this section allow a `Workspace` to be used in property-based testing
/// scenarios.
///
/// Currently, [proptest 0.9](https://docs.rs/proptest/0.9) is supported if the `proptest09` feature
/// is enabled.
impl<'g> Workspace<'g> {
    /// Returns a `Strategy` that generates random package names from this workspace.
    ///
    /// Requires the `proptest09` feature to be enabled.
    ///
    /// ## Panics
    ///
    /// Panics if there are no packages in this `Workspace`.
    pub fn prop09_name_strategy(&self) -> impl Strategy<Value = &'g str> + 'g {
        let name_list = self.name_list();
        (0..name_list.len()).prop_map(move |idx| name_list[idx].as_ref())
    }

    /// Returns a `Strategy` that generates random package IDs from this workspace.
    ///
    /// Requires the `proptest09` feature to be enabled.
    ///
    /// ## Panics
    ///
    /// Panics if there are no packages in this `Workspace`.
    pub fn prop09_id_strategy(&self) -> impl Strategy<Value = &'g PackageId> + 'g {
        let members_by_name = &self.inner.members_by_name;
        self.prop09_name_strategy()
            .prop_map(move |name| &members_by_name[name])
    }

    fn name_list(&self) -> &'g [Box<str>] {
        self.inner
            .name_list
            .get_or_init(|| self.inner.members_by_name.keys().cloned().collect())
    }
}

/// A randomly generated package resolver.
///
/// Created by `PackageGraph::prop09_resolver_strategy`. Requires the `proptest09` feature to be
/// enabled.
#[derive(Clone, Debug)]
pub struct Prop09Resolver {
    included_edges: FixedBitSet,
    check_depends_on: bool,
}

impl Prop09Resolver {
    fn new(included_edges: FixedBitSet) -> Self {
        Self {
            included_edges,
            check_depends_on: false,
        }
    }

    /// If called with true, this resolver will then verify that any links passed in are in the
    /// correct direction.
    ///
    /// Used for internal testing.
    #[cfg(test)]
    pub(crate) fn check_depends_on(&mut self, check: bool) {
        self.check_depends_on = check;
    }

    /// Returns true if the given link is accepted by this resolver.
    pub fn accept_link(&self, link: PackageLink<'_>) -> bool {
        self.included_edges.is_visited(&link.edge_ix())
    }
}

impl<'g> PackageResolver<'g> for Prop09Resolver {
    fn accept(&mut self, query: &PackageQuery<'g>, link: PackageLink<'g>) -> bool {
        if self.check_depends_on {
            assert!(
                query
                    .graph()
                    .depends_on(link.from().id(), link.to().id())
                    .expect("valid package IDs"),
                "package '{}' should depend on '{}'",
                link.from().id(),
                link.to().id()
            );
        }

        self.accept_link(link)
    }
}

pub(super) fn fixedbitset_strategy(len: usize) -> impl Strategy<Value = FixedBitSet> {
    vec(any::<bool>(), len).prop_map(|bits| {
        // FixedBitSet implements FromIterator<usize> for indexes, so just collect into it.
        bits.into_iter()
            .enumerate()
            .filter_map(|(idx, bit)| if bit { Some(idx) } else { None })
            .collect()
    })
}
