// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::debug_ignore::DebugIgnore;
use crate::graph::query_core::QueryParams;
use crate::graph::{
    DependencyDirection, PackageGraph, PackageIx, PackageLink, PackageResolver, PackageSet,
    ResolverFn,
};
use crate::petgraph_support::walk::EdgeDfs;
use crate::{Error, PackageId};
use fixedbitset::FixedBitSet;
use petgraph::prelude::*;
use petgraph::visit::{NodeFiltered, Reversed, VisitMap};

/// A query over a package graph.
///
/// This is the entry point for iterators over IDs and dependency links, and dot graph presentation.
/// A `PackageQuery` is constructed through the `query_` methods on `PackageGraph`.
#[derive(Clone, Debug)]
pub struct PackageQuery<'g> {
    // The fields are pub(super) for access within the graph module.
    pub(super) package_graph: &'g PackageGraph,
    pub(super) params: QueryParams<PackageGraph>,
}

/// ## Queries
///
/// The methods in this section create *queries* over subsets of this package graph. Use the methods
/// here to analyze transitive dependencies.
impl PackageGraph {
    /// Creates a new forward query over the entire workspace.
    ///
    /// `query_workspace` will select all workspace packages and their transitive dependencies.
    pub fn query_workspace(&self) -> PackageQuery {
        self.query_forward(self.workspace().member_ids())
            .expect("workspace packages should all be known")
    }

    /// Creates a new query that returns transitive dependencies of the given packages in the
    /// specified direction.
    ///
    /// Returns an error if any package IDs are unknown.
    pub fn query_directed<'g, 'a>(
        &'g self,
        package_ids: impl IntoIterator<Item = &'a PackageId>,
        dep_direction: DependencyDirection,
    ) -> Result<PackageQuery<'g>, Error> {
        match dep_direction {
            DependencyDirection::Forward => self.query_forward(package_ids),
            DependencyDirection::Reverse => self.query_reverse(package_ids),
        }
    }

    /// Creates a new query that returns transitive dependencies of the given packages.
    ///
    /// Returns an error if any package IDs are unknown.
    pub fn query_forward<'g, 'a>(
        &'g self,
        package_ids: impl IntoIterator<Item = &'a PackageId>,
    ) -> Result<PackageQuery<'g>, Error> {
        Ok(PackageQuery {
            package_graph: self,
            params: QueryParams::Forward(self.package_ixs(package_ids)?),
        })
    }

    /// Creates a new query that returns transitive reverse dependencies of the given packages.
    ///
    /// Returns an error if any package IDs are unknown.
    pub fn query_reverse<'g, 'a>(
        &'g self,
        package_ids: impl IntoIterator<Item = &'a PackageId>,
    ) -> Result<PackageQuery<'g>, Error> {
        Ok(PackageQuery {
            package_graph: self,
            params: QueryParams::Reverse(self.package_ixs(package_ids)?),
        })
    }
}

impl<'g> PackageQuery<'g> {
    /// Resolves this query into a set of known packages, following every link found along the
    /// way.
    ///
    /// This is the entry point for iterators.
    pub fn resolve(self) -> PackageSet<'g> {
        PackageSet::new(self.package_graph, self.params)
    }

    /// Resolves this query into a set of known packages, using the provided resolver to
    /// determine which links are followed.
    pub fn resolve_with(self, resolver: impl PackageResolver<'g>) -> PackageSet<'g> {
        PackageSet::with_resolver(self.package_graph, self.params, resolver)
    }

    /// Resolves this query into a set of known packages, using the provided resolver function
    /// to determine which links are followed.
    pub fn resolve_with_fn(self, resolver_fn: impl Fn(PackageLink<'g>) -> bool) -> PackageSet<'g> {
        self.resolve_with(ResolverFn(resolver_fn))
    }
}

/// An iterator over dependency links.
///
/// The items returned are of type `PackageLink<'g>`. Returned by `PackageQuery::into_iter_ids`.
#[derive(Clone, Debug)]
pub struct IntoIterLinks<'g> {
    package_graph: DebugIgnore<&'g PackageGraph>,
    reachable: Option<FixedBitSet>,
    edge_dfs: EdgeDfs<EdgeIndex<PackageIx>, NodeIndex<PackageIx>, FixedBitSet>,
    direction: DependencyDirection,
}

impl<'g> IntoIterLinks<'g> {
    /// Returns the direction the iteration is happening in.
    pub fn direction(&self) -> DependencyDirection {
        self.direction
    }

    fn next_triple(
        &mut self,
    ) -> Option<(
        NodeIndex<PackageIx>,
        NodeIndex<PackageIx>,
        EdgeIndex<PackageIx>,
    )> {
        use DependencyDirection::*;

        // This code dynamically switches over all the possible ways to iterate over dependencies.
        // Alternatives would be to either have separate types for all the different sorts of
        // queries (won't get unified type that way) and/or to use a trait object/dynamic iterator
        // (this approach is probably simpler, allocates less, plus there are some lifetime issues
        // with the way petgraph's traits work).
        match (&self.reachable, self.direction) {
            (Some(reachable), Forward) => self.edge_dfs.next(&NodeFiltered::from_fn(
                self.package_graph.dep_graph(),
                |ix| reachable.is_visited(&ix),
            )),
            (Some(reachable), Reverse) => {
                // As of petgraph 0.4.13, FilterNode is not implemented for &FixedBitSet, only for
                // FixedBitSet. This should be fixable upstream, but use a callback for now.
                // (LLVM should be able to optimize this.)
                self.edge_dfs
                    .next(&NodeFiltered::from_fn(
                        Reversed(self.package_graph.dep_graph()),
                        |ix| reachable.is_visited(&ix),
                    ))
                    .map(|(source_idx, target_idx, edge_idx)| {
                        // Flip the source and target around if this is a reversed graph, since the
                        // 'from' and 'to' are always right way up.
                        (target_idx, source_idx, edge_idx)
                    })
            }
            (None, Forward) => self.edge_dfs.next(self.package_graph.dep_graph()),
            (None, Reverse) => self
                .edge_dfs
                .next(Reversed(self.package_graph.dep_graph()))
                .map(|(source_idx, target_idx, edge_idx)| {
                    // Flip the source and target around if this is a reversed graph, since the
                    // 'from' and 'to' are always right way up.
                    (target_idx, source_idx, edge_idx)
                }),
        }
    }
}

impl<'g> Iterator for IntoIterLinks<'g> {
    type Item = PackageLink<'g>;

    fn next(&mut self) -> Option<Self::Item> {
        let next_triple = self.next_triple();

        next_triple.map(|(source_idx, target_idx, edge_idx)| {
            self.package_graph.edge_to_link(
                source_idx,
                target_idx,
                &self.package_graph.dep_graph()[edge_idx],
            )
        })
    }
}
