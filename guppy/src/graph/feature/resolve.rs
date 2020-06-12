// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::debug_ignore::DebugIgnore;
use crate::graph::feature::{
    CrossLink, FeatureEdge, FeatureFilter, FeatureGraph, FeatureId, FeatureList, FeatureMetadata,
    FeatureQuery, FeatureResolver,
};
use crate::graph::resolve_core::ResolveCore;
use crate::graph::{DependencyDirection, PackageMetadata, PackageSet};
use crate::petgraph_support::IxBitSet;
use crate::{Error, PackageId};
use fixedbitset::FixedBitSet;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;

impl<'g> FeatureGraph<'g> {
    /// Creates a new `FeatureSet` consisting of all members of this feature graph.
    ///
    /// This will include features that aren't depended on by any workspace packages.
    ///
    /// In most situations, `query_workspace().resolve()` is preferred. Use `resolve_all` if you
    /// know you need parts of the graph that aren't accessible from the workspace.
    pub fn resolve_all(&self) -> FeatureSet<'g> {
        FeatureSet {
            graph: DebugIgnore(*self),
            core: ResolveCore::all_nodes(self.dep_graph()),
        }
    }

    /// Creates a new `FeatureSet` consisting of all packages in this `PackageSet`, subject to the
    /// provided filter.
    pub fn resolve_packages(
        &self,
        packages: &PackageSet<'_>,
        filter: impl FeatureFilter<'g>,
    ) -> FeatureSet<'g> {
        let included: IxBitSet = self.feature_ixs_for_package_ixs_filtered(
            // The direction of iteration doesn't matter.
            packages.ixs(DependencyDirection::Forward),
            filter,
        );
        FeatureSet {
            graph: DebugIgnore(*self),
            core: ResolveCore::from_included(included),
        }
    }
}

/// A set of resolved feature IDs in a feature graph.
///
/// Created by `FeatureQuery::resolve` or the `FeatureGraph::resolve_` methods.
#[derive(Clone, Debug)]
pub struct FeatureSet<'g> {
    graph: DebugIgnore<FeatureGraph<'g>>,
    core: ResolveCore<FeatureGraph<'g>>,
}

impl<'g> FeatureSet<'g> {
    pub(super) fn new(query: FeatureQuery<'g>) -> Self {
        let graph = query.graph;
        Self {
            graph: DebugIgnore(graph),
            core: ResolveCore::new(graph.dep_graph(), query.params),
        }
    }

    pub(super) fn with_resolver(
        query: FeatureQuery<'g>,
        mut resolver: impl FeatureResolver<'g>,
    ) -> Self {
        let graph = query.graph;
        let params = query.params.clone();
        Self {
            graph: DebugIgnore(graph),
            core: ResolveCore::with_edge_filter(graph.dep_graph(), params, |edge| {
                match graph.edge_to_cross_link(
                    edge.source(),
                    edge.target(),
                    edge.id(),
                    Some(edge.weight()),
                ) {
                    Some(cross_link) => resolver.accept(&query, cross_link),
                    None => {
                        // Feature links within the same package are always followed.
                        true
                    }
                }
            }),
        }
    }

    #[allow(dead_code)]
    pub(super) fn from_included(graph: FeatureGraph<'g>, included: FixedBitSet) -> Self {
        Self {
            graph: DebugIgnore(graph),
            core: ResolveCore::from_included(included),
        }
    }

    /// Returns the `FeatureGraph` that this feature set was computed against.
    pub fn graph(&self) -> &FeatureGraph<'g> {
        &self.graph.0
    }

    /// Returns the number of feature IDs in this set.
    pub fn len(&self) -> usize {
        self.core.len()
    }

    /// Returns true if no feature IDs were resolved in this set.
    pub fn is_empty(&self) -> bool {
        self.core.is_empty()
    }

    /// Returns true if this set contains the given feature ID.
    ///
    /// Returns an error if this feature ID was unknown.
    pub fn contains<'a>(&self, feature_id: impl Into<FeatureId<'a>>) -> Result<bool, Error> {
        Ok(self
            .core
            .contains(self.graph.feature_ix(feature_id.into())?))
    }

    // ---
    // Set operations
    // ---

    /// Returns a `FeatureSet` that contains all packages present in at least one of `self`
    /// and `other`.
    ///
    /// ## Panics
    ///
    /// Panics if the package graphs associated with `self` and `other` don't match.
    pub fn union(&self, other: &Self) -> Self {
        assert!(
            ::std::ptr::eq(self.graph.package_graph, self.graph.package_graph),
            "package graphs passed into union() match"
        );
        let mut res = self.clone();
        res.core.union_with(&other.core);
        res
    }

    /// Returns a `FeatureSet` that contains all packages present in both `self` and `other`.
    ///
    /// ## Panics
    ///
    /// Panics if the package graphs associated with `self` and `other` don't match.
    pub fn intersection(&self, other: &Self) -> Self {
        assert!(
            ::std::ptr::eq(self.graph.package_graph, self.graph.package_graph),
            "package graphs passed into intersection() match"
        );
        let mut res = self.clone();
        res.core.intersect_with(&other.core);
        res
    }

    /// Returns a `FeatureSet` that contains all packages present in `self` but not `other`.
    ///
    /// ## Panics
    ///
    /// Panics if the package graphs associated with `self` and `other` don't match.
    pub fn difference(&self, other: &Self) -> Self {
        assert!(
            ::std::ptr::eq(self.graph.package_graph, self.graph.package_graph),
            "package graphs passed into difference() match"
        );
        Self {
            graph: self.graph,
            core: self.core.difference(&other.core),
        }
    }

    /// Returns a `FeatureSet` that contains all packages present in exactly one of `self` and
    /// `other`.
    ///
    /// ## Panics
    ///
    /// Panics if the package graphs associated with `self` and `other` don't match.
    pub fn symmetric_difference(&self, other: &Self) -> Self {
        assert!(
            ::std::ptr::eq(self.graph.package_graph, self.graph.package_graph),
            "package graphs passed into symmetric_difference() match"
        );
        let mut res = self.clone();
        res.core.symmetric_difference_with(&other.core);
        res
    }

    // ---
    // Queries around packages
    // ---

    /// Returns a list of features present for this package, or `None` if this package is not
    /// present in the feature set.
    ///
    /// Returns an error if the package ID was unknown.
    pub fn features_for(&self, package_id: &PackageId) -> Result<Option<FeatureList<'g>>, Error> {
        let package = self.graph.package_graph.metadata(package_id)?;
        Ok(self.features_for_package_impl(package))
    }

    /// Converts this `FeatureSet` into a `PackageSet` containing all packages with any selected
    /// features (including the "base" feature).
    pub fn to_package_set(&self) -> PackageSet<'g> {
        let included: IxBitSet = self
            .core
            .included
            .ones()
            .map(|feature_ix| {
                self.graph
                    .package_ix_for_feature_ix(NodeIndex::new(feature_ix))
            })
            .collect();
        PackageSet::from_included(self.graph.package_graph, included.0)
    }

    // ---
    // Iterators
    // ---

    /// Iterates over feature IDs, in topological order in the direction specified.
    ///
    /// ## Cycles
    ///
    /// The packages within a dependency cycle will be returned in arbitrary order, but overall
    /// topological order will be maintained.
    pub fn feature_ids<'a>(
        &'a self,
        direction: DependencyDirection,
    ) -> impl Iterator<Item = FeatureId<'g>> + ExactSizeIterator + 'a {
        let graph = self.graph;
        self.core
            .topo(graph.sccs(), direction)
            .map(move |feature_ix| {
                FeatureId::from_node(graph.package_graph(), &graph.dep_graph()[feature_ix])
            })
    }

    /// Iterates over feature metadatas, in topological order in the direction specified.
    ///
    /// ## Cycles
    ///
    /// The packages within a dependency cycle will be returned in arbitrary order, but overall
    /// topological order will be maintained.
    pub fn features<'a>(
        &'a self,
        direction: DependencyDirection,
    ) -> impl Iterator<Item = FeatureMetadata<'g>> + ExactSizeIterator + 'a {
        let graph = self.graph;
        self.core
            .topo(graph.sccs(), direction)
            .map(move |feature_ix| {
                graph
                    .metadata_for_node(graph.dep_graph()[feature_ix])
                    .expect("feature node should be known")
            })
    }

    /// Iterates over package metadatas and their corresponding features, in topological order in
    /// the direction specified.
    ///
    /// ## Cycles
    ///
    /// The packages within a dependency cycle will be returned in arbitrary order, but overall
    /// topological order will be maintained.
    pub fn packages_with_features<'a>(
        &'a self,
        direction: DependencyDirection,
    ) -> impl Iterator<Item = FeatureList<'g>> + 'a {
        let package_graph = self.graph.package_graph;

        // Use the package graph's SCCs for the topo order guarantee.
        package_graph
            .sccs()
            .node_iter(direction.into())
            .filter_map(move |package_ix| {
                let package_id = &package_graph.dep_graph()[package_ix];
                let package = package_graph
                    .metadata(package_id)
                    .expect("valid package ID");
                self.features_for_package_impl(package)
            })
    }

    /// Returns the set of "root feature" IDs in the specified direction.
    ///
    /// * If direction is Forward, return the set of feature IDs that do not have any dependencies
    ///   within the selected graph.
    /// * If direction is Reverse, return the set of feature IDs that do not have any dependents
    ///   within the selected graph.
    ///
    /// ## Cycles
    ///
    /// If a root consists of a dependency cycle, all the packages in it will be returned in
    /// arbitrary order.
    pub fn root_ids<'a>(
        &'a self,
        direction: DependencyDirection,
    ) -> impl Iterator<Item = FeatureId<'g>> + ExactSizeIterator + 'a {
        let dep_graph = self.graph.dep_graph();
        let package_graph = self.graph.package_graph;
        self.core
            .roots(dep_graph, self.graph.sccs(), direction)
            .into_iter()
            .map(move |feature_ix| FeatureId::from_node(package_graph, &dep_graph[feature_ix]))
    }

    /// Returns the set of "root feature" metadatas in the specified direction.
    ///
    /// * If direction is Forward, return the set of metadatas that do not have any dependencies
    ///   within the selected graph.
    /// * If direction is Reverse, return the set of metadatas that do not have any dependents
    ///   within the selected graph.
    ///
    /// ## Cycles
    ///
    /// If a root consists of a dependency cycle, all the packages in it will be returned in
    /// arbitrary order.}
    pub fn root_features<'a>(
        &'a self,
        direction: DependencyDirection,
    ) -> impl Iterator<Item = FeatureMetadata<'g>> + 'a {
        let feature_graph = self.graph;
        self.core
            .roots(feature_graph.dep_graph(), feature_graph.sccs(), direction)
            .into_iter()
            .map(move |feature_ix| {
                feature_graph
                    .metadata_for_node(feature_graph.dep_graph()[feature_ix])
                    .expect("feature node should be known")
            })
    }

    /// Creates an iterator over `CrossLink` instances in the direction specified.
    ///
    /// ## Cycles
    ///
    /// The links in a dependency cycle may be returned in arbitrary order.
    pub fn cross_links<'a>(
        &'a self,
        direction: DependencyDirection,
    ) -> impl Iterator<Item = CrossLink<'g>> + 'a {
        let graph = self.graph;
        self.core
            .links(graph.dep_graph(), graph.sccs(), direction)
            .filter_map(move |(source_ix, target_ix, edge_ix)| {
                graph.edge_to_cross_link(source_ix, target_ix, edge_ix, None)
            })
    }

    // ---
    // Helper methods
    // ---

    fn features_for_package_impl<'a>(
        &'a self,
        package: PackageMetadata<'g>,
    ) -> Option<FeatureList<'g>> {
        let dep_graph = self.graph.dep_graph();
        let core = &self.core;

        let mut features = self
            .graph
            .feature_ixs_for_package_ix(package.package_ix())
            .filter_map(|feature_ix| {
                if core.contains(feature_ix) {
                    Some(FeatureId::node_to_feature(package, &dep_graph[feature_ix]))
                } else {
                    None
                }
            })
            .peekable();
        if features.peek().is_some() {
            // At least one feature was returned.
            Some(FeatureList::new(package, features))
        } else {
            None
        }
    }

    // Currently a helper for debugging -- will be made public in the future.
    #[doc(hidden)]
    pub fn links<'a>(
        &'a self,
        direction: DependencyDirection,
    ) -> impl Iterator<Item = (FeatureId<'g>, FeatureId<'g>, &'g FeatureEdge)> + 'a {
        let feature_graph = self.graph;

        self.core
            .links(feature_graph.dep_graph(), feature_graph.sccs(), direction)
            .map(move |(source_ix, target_ix, edge_ix)| {
                (
                    FeatureId::from_node(
                        feature_graph.package_graph(),
                        &feature_graph.dep_graph()[source_ix],
                    ),
                    FeatureId::from_node(
                        feature_graph.package_graph(),
                        &feature_graph.dep_graph()[target_ix],
                    ),
                    &feature_graph.dep_graph()[edge_ix],
                )
            })
    }
}
