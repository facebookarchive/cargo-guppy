// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{
    debug_ignore::DebugIgnore,
    graph::{
        cargo::{CargoOptions, CargoSet},
        feature::{
            CrossLink, FeatureEdge, FeatureGraph, FeatureId, FeatureList, FeatureMetadata,
            FeatureQuery, FeatureResolver,
        },
        resolve_core::ResolveCore,
        DependencyDirection, FeatureGraphSpec, FeatureIx, PackageIx, PackageMetadata, PackageSet,
    },
    petgraph_support::IxBitSet,
    sorted_set::SortedSet,
    Error, PackageId,
};
use fixedbitset::FixedBitSet;
use petgraph::{graph::NodeIndex, visit::EdgeRef};

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

    /// Creates a new, empty `FeatureSet` associated with this feature graph.
    pub fn resolve_none(&self) -> FeatureSet<'g> {
        FeatureSet {
            graph: DebugIgnore(*self),
            core: ResolveCore::empty(),
        }
    }

    /// Creates a new `FeatureSet` consisting of the specified feature IDs.
    ///
    /// Returns an error if any feature IDs are unknown.
    pub fn resolve_ids<'a>(
        &self,
        feature_ids: impl IntoIterator<Item = impl Into<FeatureId<'a>>>,
    ) -> Result<FeatureSet<'g>, Error> {
        Ok(FeatureSet {
            graph: DebugIgnore(*self),
            core: ResolveCore::from_included::<IxBitSet>(
                self.feature_ixs(feature_ids.into_iter().map(|feature| feature.into()))?,
            ),
        })
    }
}

/// A set of resolved feature IDs in a feature graph.
///
/// Created by `FeatureQuery::resolve`, the `FeatureGraph::resolve_` methods, or from
/// `PackageSet::to_feature_set`.
#[derive(Clone, Debug)]
pub struct FeatureSet<'g> {
    graph: DebugIgnore<FeatureGraph<'g>>,
    core: ResolveCore<FeatureGraphSpec>,
}

assert_covariant!(FeatureSet);

impl<'g> FeatureSet<'g> {
    pub(super) fn new(query: FeatureQuery<'g>) -> Self {
        let graph = query.graph;
        Self {
            graph,
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
            graph,
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
    pub(in crate::graph) fn from_included(
        graph: FeatureGraph<'g>,
        included: impl Into<FixedBitSet>,
    ) -> Self {
        Self {
            graph: DebugIgnore(graph),
            core: ResolveCore::from_included(included.into()),
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

    /// Returns true if this set contains this package.
    ///
    /// Returns an error if this package ID was unknown.
    pub fn contains_package(&self, package_id: &PackageId) -> Result<bool, Error> {
        let package = self.graph.package_graph.metadata(package_id)?;
        Ok(self
            .graph
            .feature_ixs_for_package_ix(package.package_ix())
            .any(|feature_ix| self.core.contains(feature_ix)))
    }

    /// Creates a new `FeatureQuery` from this set in the specified direction.
    ///
    /// This is equivalent to constructing a query from all the feature IDs in this set.
    pub fn to_feature_query(&self, direction: DependencyDirection) -> FeatureQuery<'g> {
        let feature_ixs = SortedSet::new(
            self.core
                .included
                .ones()
                .map(NodeIndex::new)
                .collect::<Vec<_>>(),
        );
        self.graph.query_from_parts(feature_ixs, direction)
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
    // Cargo set creation
    // ---

    /// Converts this feature set into a Cargo set, simulating a Cargo build for it.
    ///
    /// The feature set is expected to be entirely within the workspace. Its behavior outside the
    /// workspace isn't defined and may be surprising.
    ///
    /// Returns an error if the `CargoOptions` weren't valid in some way (for example if an omitted
    /// package ID wasn't known to this graph.)
    pub fn into_cargo_set(self, opts: &CargoOptions<'_>) -> Result<CargoSet<'g>, Error> {
        let features_only = self.graph.resolve_none();
        CargoSet::new(self, features_only, opts)
    }

    // ---
    // Iterators
    // ---

    /// Iterates over feature IDs, in topological order in the direction specified.
    ///
    /// ## Cycles
    ///
    /// The features within a dependency cycle will be returned in non-dev order. When the direction
    /// is forward, if feature Foo has a dependency on Bar, and Bar has a cyclic dev-dependency on
    /// Foo, then Foo is returned before Bar.
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
    /// The features within a dependency cycle will be returned in non-dev order. When the direction
    /// is forward, if feature Foo has a dependency on Bar, and Bar has a cyclic dev-dependency on
    /// Foo, then Foo is returned before Bar.
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
    /// The packages within a dependency cycle will be returned in non-dev order. When the direction
    /// is forward, if package Foo has a dependency on Bar, and Bar has a cyclic dev-dependency on
    /// Foo, then Foo is returned before Bar.
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
    /// non-dev order (when the direction is forward).
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
    /// non-dev order (when the direction is forward).
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
    /// The links in a dependency cycle will be returned in non-dev order. When the direction is
    /// forward, if feature Foo has a dependency on Bar, and Bar has a cyclic dev-dependency on Foo,
    /// then the link Foo -> Bar is returned before the link Bar -> Foo.
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

    /// Returns all the package ixs without topologically sorting them.
    pub(in crate::graph) fn ixs_unordered(
        &self,
    ) -> impl Iterator<Item = NodeIndex<FeatureIx>> + '_ {
        self.core.included.ones().map(NodeIndex::new)
    }

    /// Returns true if this feature set contains the given package ix.
    #[allow(dead_code)]
    pub(in crate::graph) fn contains_package_ix(&self, package_ix: NodeIndex<PackageIx>) -> bool {
        self.graph
            .feature_ixs_for_package_ix(package_ix)
            .any(|feature_ix| self.core.contains(feature_ix))
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

impl<'g> PartialEq for FeatureSet<'g> {
    fn eq(&self, other: &Self) -> bool {
        ::std::ptr::eq(self.graph.package_graph, other.graph.package_graph)
            && self.core == other.core
    }
}

impl<'g> Eq for FeatureSet<'g> {}
