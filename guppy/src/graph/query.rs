// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::query_core::QueryParams;
use crate::graph::{
    DependencyDirection, PackageGraph, PackageLink, PackageResolver, PackageSet, ResolverFn,
};
use crate::{Error, PackageId};

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
    /// Returns the direction the query is happening in.
    pub fn direction(&self) -> DependencyDirection {
        self.params.direction()
    }

    /// Returns true if the query starts from the given package ID.
    ///
    /// Returns `None` if this package ID is unknown.
    pub fn starts_from(&self, package_id: &PackageId) -> Option<bool> {
        Some(
            self.params
                .has_initial(self.package_graph.package_ix(package_id)?),
        )
    }

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
