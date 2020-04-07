// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::errors::FeatureGraphWarning;
use crate::graph::feature::build::FeatureGraphBuildState;
use crate::graph::feature::{Cycles, FeatureFilter};
use crate::graph::{
    DependencyDirection, FeatureIx, PackageGraph, PackageIx, PackageMetadata, TargetPredicate,
};
use crate::petgraph_support::scc::Sccs;
use crate::Error;
use cargo_metadata::PackageId;
use once_cell::sync::OnceCell;
use petgraph::algo::has_path_connecting;
use petgraph::prelude::*;
use std::collections::HashMap;
use std::iter;

// Some general notes about feature graphs:
//
// The set of features for a package is the named features (in the [features] section), plus any
// optional dependencies.
//
// An optional dependency can be either normal or build -- not dev. Note that a dependency can be
// marked optional in one section and mandatory in another. In this context, a dependency is a
// feature if it is marked as optional in any context.
//
// Features are *unified*. See the documentation in add_dependency_edges for more.
//
// There are a few ways features can be enabled. The most common is within a dependency spec. A
// feature can also be specified via the command-line. Finally, named features can specify what
// features a package depends on:
//
// ```toml
// [features]
// foo = ["a/bar", "optional-dep", "baz"]
// baz = []
// ```
//
// Feature names are unique. A named feature and an optional dep cannot have the same names.

impl PackageGraph {
    /// Returns a derived graph representing every feature of every package.
    ///
    /// The feature graph is constructed the first time this method is called. The graph is cached
    /// so that repeated calls to this method are cheap.
    #[doc(hidden)]
    pub fn feature_graph(&self) -> FeatureGraph {
        let inner = self.get_feature_graph();
        FeatureGraph {
            package_graph: self,
            inner,
        }
    }

    pub(super) fn get_feature_graph(&self) -> &FeatureGraphImpl {
        self.feature_graph
            .get_or_init(|| FeatureGraphImpl::new(self))
    }
}

/// A derived graph representing every feature of every package.
///
/// Constructed through `PackageGraph::feature_graph`.
#[derive(Clone, Copy, Debug)]
pub struct FeatureGraph<'g> {
    pub(super) package_graph: &'g PackageGraph,
    pub(super) inner: &'g FeatureGraphImpl,
}

impl<'g> FeatureGraph<'g> {
    /// Returns any non-fatal warnings encountered while constructing the feature graph.
    pub fn build_warnings(&self) -> &'g [FeatureGraphWarning] {
        &self.inner.warnings
    }

    /// Returns the `PackageGraph` from which this feature graph was constructed.
    pub fn package_graph(&self) -> &'g PackageGraph {
        self.package_graph
    }

    /// Returns the total number of (package ID, feature) combinations in this graph.
    ///
    /// Includes the "base" feature for each package.
    pub fn feature_count(&self) -> usize {
        self.dep_graph().node_count()
    }

    /// Returns the number of links in this graph.
    pub fn link_count(&self) -> usize {
        self.dep_graph().edge_count()
    }

    /// Returns metadata for the given feature ID, or `None` if the feature wasn't found.
    pub fn metadata(&self, feature_id: impl Into<FeatureId<'g>>) -> Option<FeatureMetadata<'g>> {
        let feature_id = feature_id.into();
        let inner = self.metadata_impl(feature_id)?;
        Some(FeatureMetadata { feature_id, inner })
    }

    /// Returns true if this feature is included in a package's build by default.
    ///
    /// This includes transitive dependencies of the default feature.
    ///
    /// Returns `None` if this feature ID is unknown.
    pub fn is_default_feature<'a>(&self, feature_id: impl Into<FeatureId<'a>>) -> Option<bool> {
        let feature_id = feature_id.into();
        let default_ix = self.feature_ix(
            self.package_graph
                .metadata(feature_id.package_id())?
                .default_feature_id(),
        )?;
        let feature_ix = self.feature_ix(feature_id)?;
        Some(self.feature_ix_depends_on(default_ix, feature_ix))
    }

    /// Returns true if `feature_a` depends (directly or indirectly) on `feature_b`.
    ///
    /// In other words, this returns true if `feature_b` is a (possibly transitive) dependency of
    /// `feature_a`.
    pub fn depends_on<'a>(
        &self,
        feature_a: impl Into<FeatureId<'a>>,
        feature_b: impl Into<FeatureId<'a>>,
    ) -> Result<bool, Error> {
        let feature_a = feature_a.into();
        let feature_b = feature_b.into();
        let a_ix = self.feature_ix(feature_a).ok_or_else(|| {
            let (package_id, feature) = feature_a.into();
            Error::UnknownFeatureId(package_id, feature)
        })?;
        let b_ix = self.feature_ix(feature_b).ok_or_else(|| {
            let (package_id, feature) = feature_b.into();
            Error::UnknownFeatureId(package_id, feature)
        })?;
        Ok(self.feature_ix_depends_on(a_ix, b_ix))
    }

    /// Returns information about dependency cycles.
    ///
    /// For more information, see the documentation for `Cycles`.
    pub fn cycles(&self) -> Cycles<'g> {
        Cycles::new(*self)
    }

    // ---
    // Helper methods
    // ---

    /// Returns the strongly connected components for this feature graph.
    pub(super) fn sccs(&self) -> &'g Sccs<FeatureIx> {
        self.inner.sccs.get_or_init(|| Sccs::new(&self.inner.graph))
    }

    fn metadata_impl(&self, feature_id: FeatureId<'g>) -> Option<&'g FeatureMetadataImpl> {
        let feature_node = FeatureNode::from_id(self, feature_id)?;
        self.inner.map.get(&feature_node)
    }

    pub(super) fn metadata_for_node(
        &self,
        feature_node: &FeatureNode,
    ) -> Option<FeatureMetadata<'g>> {
        let metadata_impl = self.inner.map.get(feature_node)?;
        let feature_id = FeatureId::from_node(self.package_graph, feature_node);
        Some(FeatureMetadata {
            feature_id,
            inner: metadata_impl,
        })
    }

    pub(super) fn dep_graph(&self) -> &'g Graph<FeatureNode, FeatureEdge, Directed, FeatureIx> {
        &self.inner.graph
    }

    fn feature_ix_depends_on(
        &self,
        a_ix: NodeIndex<FeatureIx>,
        b_ix: NodeIndex<FeatureIx>,
    ) -> bool {
        has_path_connecting(self.dep_graph(), a_ix, b_ix, None)
    }

    pub(super) fn feature_ixs_for_packages<'a>(
        &'a self,
        package_ixs: impl IntoIterator<Item = NodeIndex<PackageIx>> + 'a,
        filter: impl FeatureFilter<'g> + 'a,
    ) -> impl Iterator<Item = NodeIndex<FeatureIx>> + 'a {
        let mut filter = filter;
        package_ixs.into_iter().flat_map(move |package_ix| {
            let package_id = &self.package_graph.dep_graph[package_ix];
            let metadata = self
                .package_graph
                .metadata(package_id)
                .expect("package ID should have valid metadata");
            metadata
                .all_feature_nodes()
                .filter_map(|feature_node| {
                    if filter.accept(
                        self,
                        FeatureId::from_node(self.package_graph, &feature_node),
                    ) {
                        Some(
                            self.inner
                                .map
                                .get(&feature_node)
                                .expect("feature node should be valid")
                                .feature_ix,
                        )
                    } else {
                        None
                    }
                })
                // This collect() shouldn't be necessary, but Rust 1.40 complains that "captured
                // variable cannot escape `FnMut` closure body". Unclear why.
                .collect::<Vec<_>>()
        })
    }

    #[allow(dead_code)]
    pub(super) fn feature_ixs<'a, B>(
        &self,
        feature_ids: impl IntoIterator<Item = FeatureId<'g>>,
    ) -> Result<B, Error>
    where
        B: iter::FromIterator<NodeIndex<FeatureIx>>,
    {
        feature_ids
            .into_iter()
            .map(|feature_id| {
                self.feature_ix(feature_id).ok_or_else(|| {
                    let (package_id, feature) = feature_id.into();
                    Error::UnknownFeatureId(package_id, feature)
                })
            })
            .collect()
    }

    pub(super) fn feature_ix(&self, feature_id: FeatureId<'g>) -> Option<NodeIndex<FeatureIx>> {
        let metadata = self.metadata_impl(feature_id)?;
        Some(metadata.feature_ix)
    }
}

/// An identifier for a (package, feature) pair in a feature graph.
///
/// Returned by various methods on `FeatureGraph` and `FeatureSelect`.
///
/// `From` impls are available for `(&'g PackageId, &'g str)` and `(&'g PackageId, Option<&'g str>)`
/// tuples.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FeatureId<'g> {
    package_id: &'g PackageId,
    feature: Option<&'g str>,
}

impl<'g> FeatureId<'g> {
    /// Creates a new `FeatureId`.
    pub fn new(package_id: &'g PackageId, feature: &'g str) -> Self {
        Self {
            package_id,
            feature: Some(feature),
        }
    }

    /// Creates a new `FeatureId` representing the "base" feature for a package.
    pub fn base(package_id: &'g PackageId) -> Self {
        Self {
            package_id,
            feature: None,
        }
    }

    pub(super) fn from_node(package_graph: &'g PackageGraph, node: &FeatureNode) -> Self {
        let package_id = &package_graph.dep_graph[node.package_ix];
        let metadata = package_graph
            .metadata(package_id)
            .expect("package ID should have valid metadata");
        let feature = node.feature_idx.as_ref().map(|feature_idx| {
            metadata
                .features
                .get_index(*feature_idx)
                .expect("feature idx should be valid")
                .0
                .as_ref()
        });
        Self {
            package_id,
            feature,
        }
    }

    /// Returns the package ID.
    pub fn package_id(&self) -> &'g PackageId {
        self.package_id
    }

    /// Returns the name of the feature, or `None` if this is the "base" feature for this package.
    pub fn feature(&self) -> Option<&'g str> {
        self.feature
    }

    /// Returns true if this is the "base" feature for the package.
    pub fn is_base(&self) -> bool {
        self.feature.is_none()
    }
}

impl<'g> From<(&'g PackageId, &'g str)> for FeatureId<'g> {
    fn from((package_id, feature): (&'g PackageId, &'g str)) -> Self {
        FeatureId::new(package_id, feature)
    }
}

impl<'g> From<(&'g PackageId, Option<&'g str>)> for FeatureId<'g> {
    fn from((package_id, feature): (&'g PackageId, Option<&'g str>)) -> Self {
        FeatureId {
            package_id,
            feature,
        }
    }
}

impl<'g> From<FeatureId<'g>> for (PackageId, Option<String>) {
    fn from(feature_id: FeatureId<'g>) -> Self {
        (
            feature_id.package_id().clone(),
            feature_id.feature().map(|feature| feature.to_string()),
        )
    }
}

/// Metadata for a feature within a package.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FeatureMetadata<'g> {
    feature_id: FeatureId<'g>,
    inner: &'g FeatureMetadataImpl,
}

impl<'g> FeatureMetadata<'g> {
    /// Returns the feature ID corresponding to this metadata.
    pub fn feature_id(&self) -> FeatureId<'g> {
        self.feature_id
    }

    /// Returns the type of this feature.
    pub fn feature_type(&self) -> FeatureType {
        self.inner.feature_type
    }
}

/// A graph representing every possible feature of every package, and the connections between them.
#[derive(Clone, Debug)]
pub(in crate::graph) struct FeatureGraphImpl {
    pub(super) graph: Graph<FeatureNode, FeatureEdge, Directed, FeatureIx>,
    pub(super) map: HashMap<FeatureNode, FeatureMetadataImpl>,
    pub(super) warnings: Vec<FeatureGraphWarning>,
    // The strongly connected components of the feature graph. Computed on demand.
    pub(super) sccs: OnceCell<Sccs<FeatureIx>>,
}

impl FeatureGraphImpl {
    /// Creates a new `FeatureGraph` from this `PackageGraph`.
    pub(super) fn new(package_graph: &PackageGraph) -> Self {
        let mut build_state = FeatureGraphBuildState::new(package_graph);

        // The iteration order is bottom-up to allow linking up for "a/foo" style feature specs.
        for metadata in package_graph
            .select_all()
            .resolve()
            .into_metadatas(DependencyDirection::Reverse)
        {
            build_state.add_nodes(metadata);
            // into_metadatas is in topological order, so all the dependencies of this package would
            // have been added already. So named feature edges can safely be added.
            build_state.add_named_feature_edges(metadata);
        }

        // The iteration order doesn't matter here, but use bottom-up for symmetry with the previous
        // loop.
        for link in package_graph
            .select_all()
            .into_iter_links(Some(DependencyDirection::Reverse))
        {
            build_state.add_dependency_edges(link);
        }

        build_state.build()
    }
}

/// A combination of a package ID and a feature name, forming a node in a `FeatureGraph`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(in crate::graph) struct FeatureNode {
    package_ix: NodeIndex<PackageIx>,
    feature_idx: Option<usize>,
}

impl FeatureNode {
    /// Returns a new feature node.
    pub(in crate::graph) fn new(package_ix: NodeIndex<PackageIx>, feature_idx: usize) -> Self {
        Self {
            package_ix,
            feature_idx: Some(feature_idx),
        }
    }

    /// Returns a new feature node representing the base package with no features enabled.
    pub(in crate::graph) fn base(package_ix: NodeIndex<PackageIx>) -> Self {
        Self {
            package_ix,
            feature_idx: None,
        }
    }

    /// Returns a new feature node, can also be the base.
    pub(in crate::graph) fn new_opt(
        package_ix: NodeIndex<PackageIx>,
        feature_idx: Option<usize>,
    ) -> Self {
        Self {
            package_ix,
            feature_idx,
        }
    }

    fn from_id(feature_graph: &FeatureGraph<'_>, id: FeatureId<'_>) -> Option<Self> {
        let metadata = feature_graph.package_graph.metadata(id.package_id())?;
        match id.feature() {
            Some(feature_name) => Some(FeatureNode::new(
                metadata.package_ix,
                metadata.get_feature_idx(feature_name)?,
            )),
            None => Some(FeatureNode::base(metadata.package_ix)),
        }
    }

    pub(super) fn named_features<'g>(
        package: &'g PackageMetadata,
    ) -> impl Iterator<Item = Self> + 'g {
        let package_ix = package.package_ix;
        package.named_features_full().map(move |(n, _, _)| Self {
            package_ix,
            feature_idx: Some(n),
        })
    }
}

/// Information about why a feature depends on another feature.
#[derive(Clone, Debug)]
pub(in crate::graph) enum FeatureEdge {
    /// This edge is from a feature to its base package.
    FeatureToBase,
    /// This edge is present because a feature is enabled in a dependency, e.g. through:
    ///
    /// ```toml
    /// [dependencies]
    /// foo = { version = "1", features = ["a", "b"] }
    /// ```
    Dependency {
        normal: TargetPredicate,
        build: TargetPredicate,
        dev: TargetPredicate,
    },
    /// This edge is from a feature depending on other features:
    ///
    /// ```toml
    /// [features]
    /// "a" = ["b", "foo/c"]
    /// ```
    FeatureDependency,
}

/// Metadata for a particular feature node.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct FeatureMetadataImpl {
    pub(super) feature_ix: NodeIndex<FeatureIx>,
    pub(super) feature_type: FeatureType,
}

/// The type of a particular feature within a package.
///
/// Returned by `FeatureMetadata`.
#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FeatureType {
    /// This is a named feature in the `[features]` section.
    NamedFeature,
    /// This is an optional dependency.
    OptionalDep,
    /// This is the "base" package with no features enabled.
    BasePackage,
}
