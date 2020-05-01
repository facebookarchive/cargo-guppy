// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::errors::FeatureGraphWarning;
use crate::graph::feature::build::FeatureGraphBuildState;
use crate::graph::feature::{Cycles, FeatureFilter};
use crate::graph::{
    DependencyDirection, FeatureIx, PackageGraph, PackageIx, PackageLink, PackageMetadata,
    PlatformStatus, PlatformStatusImpl,
};
use crate::petgraph_support::scc::Sccs;
use crate::{DependencyKind, Error, PackageId};
use once_cell::sync::OnceCell;
use petgraph::algo::has_path_connecting;
use petgraph::prelude::*;
use petgraph::visit::IntoNodeReferences;
use std::collections::HashMap;
use std::fmt;
use std::iter;
use std::iter::FromIterator;

// Some general notes about feature graphs:
//
// The set of features for a package is the named features (in the [features] section), plus any
// optional dependencies.
//
// An optional dependency can be either normal or build -- not dev. Note that a dependency can be
// marked optional in one section and required in another. In this context, a dependency is a
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
    pub(crate) package_graph: &'g PackageGraph,
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
        let feature_node = FeatureNode::from_id(self, feature_id.into())?;
        self.metadata_for_node(feature_node)
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
    ///
    /// This also returns true if `feature_a` is the same as `feature_b`.
    pub fn depends_on<'a>(
        &self,
        feature_a: impl Into<FeatureId<'a>>,
        feature_b: impl Into<FeatureId<'a>>,
    ) -> Result<bool, Error> {
        let feature_a = feature_a.into();
        let feature_b = feature_b.into();
        let a_ix = self.feature_ix_err(feature_a)?;
        let b_ix = self.feature_ix_err(feature_b)?;
        Ok(self.feature_ix_depends_on(a_ix, b_ix))
    }

    /// Returns true if `feature_a` directly depends on `feature_b`.
    ///
    /// In other words, this returns true if `feature_a` is a direct dependency of `feature_b`.
    ///
    /// This returns false if `feature_a` is the same as `feature_b`.
    pub fn directly_depends_on<'a>(
        &self,
        feature_a: impl Into<FeatureId<'a>>,
        feature_b: impl Into<FeatureId<'a>>,
    ) -> Result<bool, Error> {
        let feature_a = feature_a.into();
        let feature_b = feature_b.into();
        let a_ix = self.feature_ix_err(feature_a)?;
        let b_ix = self.feature_ix_err(feature_b)?;
        Ok(self.dep_graph().contains_edge(a_ix, b_ix))
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

    /// Verify basic properties of the feature graph.
    #[doc(hidden)]
    pub fn verify(&self) -> Result<(), Error> {
        let feature_set = self.resolve_all();
        for cross_link in feature_set.cross_links(DependencyDirection::Forward) {
            let (from, to) = cross_link.endpoints();
            let is_any = cross_link.normal().is_present()
                || cross_link.build().is_present()
                || cross_link.dev().is_present();

            if !is_any {
                return Err(Error::FeatureGraphInternalError(format!(
                    "{} -> {}: no edge info found",
                    from.feature_id(),
                    to.feature_id()
                )));
            }
        }

        Ok(())
    }

    /// Returns the strongly connected components for this feature graph.
    pub(super) fn sccs(&self) -> &'g Sccs<FeatureIx> {
        self.inner.sccs.get_or_init(|| Sccs::new(&self.inner.graph))
    }

    fn metadata_impl(&self, feature_id: FeatureId<'g>) -> Option<&'g FeatureMetadataImpl> {
        let feature_node = FeatureNode::from_id(self, feature_id)?;
        self.metadata_impl_for_node(&feature_node)
    }

    pub(super) fn metadata_for_node(&self, node: FeatureNode) -> Option<FeatureMetadata<'g>> {
        let inner = self.metadata_impl_for_node(&node)?;
        Some(FeatureMetadata {
            graph: *self,
            node,
            inner,
        })
    }

    #[inline]
    fn metadata_impl_for_node(&self, node: &FeatureNode) -> Option<&'g FeatureMetadataImpl> {
        self.inner.map.get(node)
    }

    pub(super) fn dep_graph(&self) -> &'g Graph<FeatureNode, FeatureEdge, Directed, FeatureIx> {
        &self.inner.graph
    }

    /// If this is a cross edge, return the cross link. Otherwise, return None.
    pub(super) fn edge_to_cross_link(
        &self,
        source_ix: NodeIndex<FeatureIx>,
        target_ix: NodeIndex<FeatureIx>,
        edge_ix: EdgeIndex<FeatureIx>,
        edge: Option<&'g FeatureEdge>,
    ) -> Option<CrossLink<'g>> {
        match edge.unwrap_or_else(|| &self.dep_graph()[edge_ix]) {
            FeatureEdge::FeatureDependency | FeatureEdge::FeatureToBase => None,
            FeatureEdge::CrossPackage(inner) => {
                Some(CrossLink::new(*self, source_ix, target_ix, edge_ix, &inner))
            }
        }
    }

    fn feature_ix_depends_on(
        &self,
        a_ix: NodeIndex<FeatureIx>,
        b_ix: NodeIndex<FeatureIx>,
    ) -> bool {
        has_path_connecting(self.dep_graph(), a_ix, b_ix, None)
    }

    pub(super) fn feature_ixs_for_package_ix(
        &self,
        package_ix: NodeIndex<PackageIx>,
    ) -> impl Iterator<Item = NodeIndex<FeatureIx>> {
        let package_ix = package_ix.index();
        let base_ix = self.inner.base_ixs[package_ix].index();
        // base_ixs has (package count + 1) elements so this access is valid.
        let next_base_ix = self.inner.base_ixs[package_ix + 1].index();

        (base_ix..next_base_ix).map(NodeIndex::new)
    }

    pub(super) fn feature_ixs_for_package_ixs(
        &self,
        package_ixs: impl IntoIterator<Item = NodeIndex<PackageIx>> + 'g,
    ) -> impl Iterator<Item = NodeIndex<FeatureIx>> + 'g {
        // Create a copy of FeatureGraph that will be moved into the closure below.
        let this = *self;

        package_ixs
            .into_iter()
            .flat_map(move |package_ix| this.feature_ixs_for_package_ix(package_ix))
    }

    pub(super) fn feature_ixs_for_package_ixs_filtered<B>(
        &self,
        package_ixs: impl IntoIterator<Item = NodeIndex<PackageIx>>,
        filter: impl FeatureFilter<'g>,
    ) -> B
    where
        B: FromIterator<NodeIndex<FeatureIx>>,
    {
        let mut filter = filter;

        self.feature_ixs_for_package_ixs(package_ixs)
            .filter(|feature_ix| {
                let feature_node = &self.dep_graph()[*feature_ix];
                filter.accept(
                    &self,
                    FeatureId::from_node(self.package_graph, feature_node),
                )
            })
            .collect()
    }

    pub(in crate::graph) fn package_ix_for_feature_ix(
        &self,
        feature_ix: NodeIndex<FeatureIx>,
    ) -> NodeIndex<PackageIx> {
        let feature_node = &self.dep_graph()[feature_ix];
        feature_node.package_ix()
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
            .map(|feature_id| self.feature_ix_err(feature_id))
            .collect()
    }

    pub(super) fn feature_ix(&self, feature_id: FeatureId<'g>) -> Option<NodeIndex<FeatureIx>> {
        let metadata = self.metadata_impl(feature_id)?;
        Some(metadata.feature_ix)
    }

    pub(super) fn feature_ix_err(
        &self,
        feature_id: FeatureId<'g>,
    ) -> Result<NodeIndex<FeatureIx>, Error> {
        self.feature_ix(feature_id).ok_or_else(|| {
            let (package_id, feature) = feature_id.into();
            Error::UnknownFeatureId(package_id, feature)
        })
    }
}

/// An identifier for a (package, feature) pair in a feature graph.
///
/// Returned by various methods on `FeatureGraph` and `FeatureQuery`.
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
        let feature = Self::node_to_feature(metadata, node);
        Self {
            package_id,
            feature,
        }
    }

    pub(super) fn node_to_feature(
        metadata: PackageMetadata<'g>,
        node: &FeatureNode,
    ) -> Option<&'g str> {
        let feature_idx = node.feature_idx?;
        metadata.feature_idx_to_name(feature_idx)
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

/// The `Display` impl prints out `{package id}/feature`, or `{package id}/[base]`.
///
/// ## Examples
///
/// ```
/// use guppy::PackageId;
/// use guppy::graph::feature::FeatureId;
///
/// let package_id = PackageId::new("region 2.1.2 (registry+https://github.com/rust-lang/crates.io-index)");
///
/// assert_eq!(
///     format!("{}", FeatureId::new(&package_id, "foo")),
///     "region 2.1.2 (registry+https://github.com/rust-lang/crates.io-index)/foo"
/// );
///
/// assert_eq!(
///     format!("{}", FeatureId::base(&package_id)),
///     "region 2.1.2 (registry+https://github.com/rust-lang/crates.io-index)/[base]"
/// );
/// ```
impl<'g> fmt::Display for FeatureId<'g> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let feature_name = self.feature.unwrap_or("[base]");
        write!(f, "{}/{}", self.package_id, feature_name)
    }
}

/// Metadata for a feature within a package.
#[derive(Clone, Copy, Debug)]
pub struct FeatureMetadata<'g> {
    graph: FeatureGraph<'g>,
    node: FeatureNode,
    inner: &'g FeatureMetadataImpl,
}

impl<'g> FeatureMetadata<'g> {
    /// Returns the feature ID corresponding to this metadata.
    pub fn feature_id(&self) -> FeatureId<'g> {
        FeatureId::from_node(self.graph.package_graph, &self.node)
    }

    /// Returns the package ID corresponding to this metadata.
    pub fn package_id(&self) -> &'g PackageId {
        &self.graph.package_graph.dep_graph[self.package_ix()]
    }

    /// Returns the package metadata corresponding to this feature metadata.
    pub fn package(&self) -> PackageMetadata<'g> {
        self.graph
            .package_graph
            .metadata(self.package_id())
            .expect("valid package ID")
    }

    /// Returns the type of this feature.
    pub fn feature_type(&self) -> FeatureType {
        self.inner.feature_type
    }

    // ---
    // Helper methods
    // ---

    #[inline]
    pub(in crate::graph) fn package_ix(&self) -> NodeIndex<PackageIx> {
        self.node.package_ix
    }
}

/// A graph representing every possible feature of every package, and the connections between them.
#[derive(Clone, Debug)]
pub(in crate::graph) struct FeatureGraphImpl {
    pub(super) graph: Graph<FeatureNode, FeatureEdge, Directed, FeatureIx>,
    // base ixs consists of the base (start) feature indexes for each package.
    pub(super) base_ixs: Vec<NodeIndex<FeatureIx>>,
    pub(super) map: HashMap<FeatureNode, FeatureMetadataImpl>,
    pub(super) warnings: Vec<FeatureGraphWarning>,
    // The strongly connected components of the feature graph. Computed on demand.
    pub(super) sccs: OnceCell<Sccs<FeatureIx>>,
}

impl FeatureGraphImpl {
    /// Creates a new `FeatureGraph` from this `PackageGraph`.
    pub(super) fn new(package_graph: &PackageGraph) -> Self {
        let mut build_state = FeatureGraphBuildState::new(package_graph);

        // Graph returns its node references in order -- check this in debug builds.
        let mut prev_ix = None;
        for (package_ix, package_id) in package_graph.dep_graph.node_references() {
            if let Some(prev_ix) = prev_ix {
                debug_assert_eq!(package_ix.index(), prev_ix + 1, "package ixs are in order");
            }
            prev_ix = Some(package_ix.index());

            let metadata = package_graph
                .metadata(package_id)
                .expect("valid package ID");
            build_state.add_nodes(metadata);
        }

        build_state.end_nodes();

        // The choice of bottom-up for this loop and the next is pretty arbitrary.
        for metadata in package_graph
            .resolve_all()
            .packages(DependencyDirection::Reverse)
        {
            build_state.add_named_feature_edges(metadata);
        }

        for link in package_graph
            .resolve_all()
            .links(DependencyDirection::Reverse)
        {
            build_state.add_dependency_edges(link);
        }

        build_state.build()
    }
}

/// A feature dependency across packages.
///
/// This is currently an opaque type -- it will be filled out in the future.
#[derive(Copy, Clone, Debug)]
pub struct CrossLink<'g> {
    graph: FeatureGraph<'g>,
    from: &'g FeatureMetadataImpl,
    to: &'g FeatureMetadataImpl,
    edge_ix: EdgeIndex<FeatureIx>,
    inner: &'g CrossLinkImpl,
}

impl<'g> CrossLink<'g> {
    #[allow(dead_code)]
    pub(super) fn new(
        graph: FeatureGraph<'g>,
        source_ix: NodeIndex<FeatureIx>,
        target_ix: NodeIndex<FeatureIx>,
        edge_ix: EdgeIndex<FeatureIx>,
        inner: &'g CrossLinkImpl,
    ) -> Self {
        let dep_graph = graph.dep_graph();
        Self {
            graph,
            from: graph
                .metadata_impl_for_node(&dep_graph[source_ix])
                .expect("valid source ix"),
            to: graph
                .metadata_impl_for_node(&dep_graph[target_ix])
                .expect("valid target ix"),
            edge_ix,
            inner,
        }
    }

    /// Returns the feature which depends on the `to` feature.
    pub fn from(&self) -> FeatureMetadata<'g> {
        FeatureMetadata {
            graph: self.graph,
            node: self.graph.dep_graph()[self.from.feature_ix],
            inner: self.from,
        }
    }

    /// Returns the feature which is depended on by the `from` feature.
    pub fn to(&self) -> FeatureMetadata<'g> {
        FeatureMetadata {
            graph: self.graph,
            node: self.graph.dep_graph()[self.to.feature_ix],
            inner: self.to,
        }
    }

    /// Returns the endpoints as a pair of features `(from, to)`.
    pub fn endpoints(&self) -> (FeatureMetadata<'g>, FeatureMetadata<'g>) {
        (self.from(), self.to())
    }

    /// Returns details about this feature dependency from the `[dependencies]` section.
    pub fn normal(&self) -> PlatformStatus<'g> {
        PlatformStatus::new(&self.inner.normal)
    }

    /// Returns details about this feature dependency from the `[build-dependencies]` section.
    pub fn build(&self) -> PlatformStatus<'g> {
        PlatformStatus::new(&self.inner.build)
    }

    /// Returns details about this feature dependency from the `[dev-dependencies]` section.
    pub fn dev(&self) -> PlatformStatus<'g> {
        PlatformStatus::new(&self.inner.dev)
    }

    /// Returns details about this feature dependency from the section specified by the given
    /// dependency kind.
    pub fn status_for_kind(&self, kind: DependencyKind) -> PlatformStatus<'g> {
        match kind {
            DependencyKind::Normal => self.normal(),
            DependencyKind::Build => self.build(),
            DependencyKind::Development => self.dev(),
        }
    }

    /// Returns true if this edge is dev-only, i.e. code from this edge will not be included in
    /// normal builds.
    pub fn dev_only(&self) -> bool {
        self.normal().is_never() && self.build().is_never()
    }

    /// Returns the `PackageLink` from which this `CrossLink` was derived.
    pub fn package_link(&self) -> PackageLink<'g> {
        let from_node = self.graph.dep_graph()[self.from.feature_ix];
        let to_node = self.graph.dep_graph()[self.to.feature_ix];
        self.graph.package_graph.edge_to_link(
            from_node.package_ix,
            to_node.package_ix,
            self.inner.package_edge_ix,
            None,
        )
    }
}

// ---

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
                metadata.package_ix(),
                metadata.get_feature_idx(feature_name)?,
            )),
            None => Some(FeatureNode::base(metadata.package_ix())),
        }
    }

    pub(super) fn named_features<'g>(
        package: PackageMetadata<'g>,
    ) -> impl Iterator<Item = Self> + 'g {
        let package_ix = package.package_ix();
        package.named_features_full().map(move |(n, _, _)| Self {
            package_ix,
            feature_idx: Some(n),
        })
    }

    pub(in crate::graph) fn package_ix(&self) -> NodeIndex<PackageIx> {
        self.package_ix
    }
}

/// Information about why a feature depends on another feature.
#[derive(Clone, Debug)]
pub(crate) enum FeatureEdge {
    /// This edge is from a feature to its base package.
    FeatureToBase,
    /// This edge is present because a feature is enabled in a dependency, e.g. through:
    ///
    /// ```toml
    /// [dependencies]
    /// foo = { version = "1", features = ["a", "b"] }
    /// ```
    CrossPackage(CrossLinkImpl),
    /// This edge is from a feature depending on other features:
    ///
    /// ```toml
    /// [features]
    /// "a" = ["b", "foo/c"]
    /// ```
    FeatureDependency,
}

#[derive(Clone, Debug)]
pub(crate) struct CrossLinkImpl {
    pub(super) package_edge_ix: EdgeIndex<PackageIx>,
    pub(super) normal: PlatformStatusImpl,
    pub(super) build: PlatformStatusImpl,
    pub(super) dev: PlatformStatusImpl,
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
