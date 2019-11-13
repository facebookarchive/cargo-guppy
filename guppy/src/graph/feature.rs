// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::errors::{FeatureBuildStage, FeatureGraphWarning};
use crate::graph::{
    DependencyDirection, DependencyLink, FeatureIx, PackageGraph, PackageIx, PackageMetadata,
};
use petgraph::prelude::*;
use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
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
    pub fn feature_graph(&self) -> FeatureGraph {
        let inner = self.get_feature_graph();
        FeatureGraph { inner }
    }

    pub(super) fn get_feature_graph(&self) -> &FeatureGraphImpl {
        self.feature_graph
            .get_or_init(|| FeatureGraphImpl::new(self))
    }
}

/// A derived graph representing every feature of every package.
///
/// Constructed through `PackageGraph::feature_graph`.
pub struct FeatureGraph<'g> {
    inner: &'g FeatureGraphImpl,
}

impl<'g> FeatureGraph<'g> {
    /// Returns any non-fatal warnings encountered while constructing the feature graph.
    pub fn build_warnings(&self) -> &[FeatureGraphWarning] {
        &self.inner.warnings
    }

    // TODO: more methods
}

/// A graph representing every possible feature of every package, and the connections between them.
#[derive(Clone, Debug)]
pub(super) struct FeatureGraphImpl {
    graph: Graph<FeatureNode, FeatureEdge, Directed, FeatureIx>,
    map: HashMap<FeatureNode, FeatureMetadata>,
    warnings: Vec<FeatureGraphWarning>,
}

impl FeatureGraphImpl {
    /// Creates a new `FeatureGraph` from this `PackageGraph`.
    pub(super) fn new(package_graph: &PackageGraph) -> Self {
        let mut build_state = FeatureGraphBuildState::new(package_graph);

        // The iteration order is bottom-up to allow linking up for "a/foo" style feature specs.
        for metadata in package_graph
            .select_all()
            .into_iter_metadatas(Some(DependencyDirection::Reverse))
        {
            build_state.add_nodes(metadata);
            // into_iter_metadatas is in topological order, so all the dependencies of this package
            // would have been added already. So named feature edges can safely be added.
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
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct FeatureNode {
    node_idx: NodeIndex<PackageIx>,
    feature: Option<Box<str>>,
}

impl FeatureNode {
    /// Returns a new feature node.
    fn new(node_idx: NodeIndex<PackageIx>, feature: impl Into<Box<str>>) -> Self {
        Self {
            node_idx,
            feature: Some(feature.into()),
        }
    }

    /// Returns a new feature node representing the base package with no features enabled.
    fn base(node_idx: NodeIndex<PackageIx>) -> Self {
        Self {
            node_idx,
            feature: None,
        }
    }

    fn named_features<'g>(
        package: &'g PackageMetadata,
    ) -> impl Iterator<Item = Self> + ExactSizeIterator + 'g {
        let node_idx = package.node_idx;
        package.named_features().map(move |feature| Self {
            node_idx,
            feature: Some(feature.into()),
        })
    }
}

// Borrowed form of a FeatureNode.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct FeatureNodeRef<'a> {
    node_idx: NodeIndex<PackageIx>,
    feature: Option<&'a str>,
}

impl<'a> FeatureNodeRef<'a> {
    fn new(node_idx: NodeIndex<PackageIx>, feature: &'a str) -> Self {
        Self {
            node_idx,
            feature: Some(feature),
        }
    }

    fn base(node_idx: NodeIndex<PackageIx>) -> Self {
        Self {
            node_idx,
            feature: None,
        }
    }

    fn base_and_all_features(
        node_idx: NodeIndex<PackageIx>,
        features: impl IntoIterator<Item = &'a str> + 'a,
    ) -> impl Iterator<Item = FeatureNodeRef<'a>> + 'a {
        iter::once(Self::base(node_idx)).chain(features.into_iter().map(move |feature| Self {
            node_idx,
            feature: Some(feature),
        }))
    }
}

/// Information about why a feature depends on another feature.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) enum FeatureEdge {
    /// This edge is from a feature to its base package.
    FeatureToBase,
    /// This edge is present because a feature is enabled in a dependency, e.g. through:
    ///
    /// ```toml
    /// [dependencies]
    /// foo = { version = "1", features = ["a", "b"] }
    /// ```
    Dependency {
        normal: bool,
        build: bool,
        dev: bool,
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
pub(super) struct FeatureMetadata {
    feature_idx: NodeIndex<FeatureIx>,
    feature_type: FeatureType,
}

/// The type of a particular feature.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub(super) enum FeatureType {
    /// This is a named feature in the `[features]` section.
    NamedFeature,
    /// This is an optional dependency.
    OptionalDep,
    /// This is the "base" package with no features enabled.
    BasePackage,
}

#[derive(Debug)]
struct FeatureGraphBuildState<'g> {
    package_graph: &'g PackageGraph,
    graph: Graph<FeatureNode, FeatureEdge, Directed, FeatureIx>,
    map: HashMap<FeatureNode, FeatureMetadata>,
    warnings: Vec<FeatureGraphWarning>,
}

impl<'g> FeatureGraphBuildState<'g> {
    fn new(package_graph: &'g PackageGraph) -> Self {
        Self {
            package_graph,
            // Each package corresponds to at least one feature ID.
            graph: Graph::with_capacity(
                package_graph.package_count(),
                package_graph.package_count(),
            ),
            map: HashMap::with_capacity(package_graph.package_count()),
            warnings: vec![],
        }
    }

    /// Add nodes for every feature in this package + the base package, and add edges from every
    /// feature to the base package.
    fn add_nodes(&mut self, package: &'g PackageMetadata) {
        let base_node = FeatureNode::base(package.node_idx);
        let base_idx = self.add_node(base_node, FeatureType::BasePackage);
        FeatureNode::named_features(package).for_each(|feature_node| {
            let feature_idx = self.add_node(feature_node, FeatureType::NamedFeature);
            self.graph
                .update_edge(feature_idx, base_idx, FeatureEdge::FeatureToBase);
        });

        package.optional_deps.iter().for_each(|dep_name| {
            let dep_idx = self.add_node(
                FeatureNode::new(package.node_idx, dep_name.clone()),
                FeatureType::OptionalDep,
            );
            self.graph
                .update_edge(dep_idx, base_idx, FeatureEdge::FeatureToBase);
        });
    }

    fn add_named_feature_edges(&mut self, metadata: &PackageMetadata) {
        let dep_name_to_idx: HashMap<_, _> = self
            .package_graph
            .dep_links(metadata.id())
            .expect("valid metadata")
            .map(|link| (link.edge.dep_name(), link.to.node_idx))
            .collect();

        metadata
            .features
            .iter()
            .for_each(|(named_feature, feature_deps)| {
                let from_node = FeatureNodeRef::new(metadata.node_idx, named_feature);
                let to_nodes: Vec<_> = feature_deps
                    .iter()
                    .filter_map(|feature_dep| {
                        let (dep_name, to_feature_name) = Self::split_feature_dep(feature_dep);
                        match dep_name {
                            Some(dep_name) => {
                                match dep_name_to_idx.get(dep_name) {
                                    Some(dep_idx) => {
                                        Some(FeatureNodeRef::new(*dep_idx, to_feature_name))
                                    }
                                    None => {
                                        // This is an unresolved feature -- it won't be included as
                                        // a dependency.
                                        // XXX revisit this if we start modeling unresolved
                                        // dependencies.
                                        None
                                    }
                                }
                            }
                            None => Some(FeatureNodeRef::new(metadata.node_idx, to_feature_name)),
                        }
                    })
                    .collect();
                // Don't create a map to the base 'from' node since it is already created in
                // add_nodes.
                self.add_edges(from_node, to_nodes, FeatureEdge::FeatureDependency, || {
                    FeatureBuildStage::AddNamedFeatureEdges {
                        package_id: metadata.id().clone(),
                        from_feature: named_feature.clone(),
                    }
                });
            })
    }

    /// Split a feature dep into package and feature names.
    ///
    /// "foo" -> (None, "foo")
    /// "dep/foo" -> (Some("dep"), "foo")
    fn split_feature_dep(feature_dep: &str) -> (Option<&str>, &str) {
        let mut rsplit = feature_dep.rsplitn(2, '/');
        let to_feature_name = rsplit
            .next()
            .expect("rsplitn should return at least one element");
        let dep_name = rsplit.next();

        (dep_name, to_feature_name)
    }

    fn add_dependency_edges(&mut self, link: DependencyLink<'_>) {
        let DependencyLink { from, to, edge } = link;

        // Sometimes the same package is depended on separately in different sections like so:
        //
        // bar/Cargo.toml:
        //
        // [dependencies]
        // foo = { version = "1", features = ["a"] }
        //
        // [build-dependencies]
        // foo = { version = "1", features = ["b"] }
        //
        // Now if you have a crate 'baz' with:
        //
        // [dependencies]
        // bar = { path = "../bar" }
        //
        // ... what features would you expect foo to be built with? You might expect it to just
        // be built with "a", but as it turns out Cargo actually *unifies* the features, such
        // that foo is built with both "a" and "b".
        //
        // There's one nuance: Cargo doesn't consider dev-dependencies of non-workspace
        // packages. So if 'from' is a workspace package, look at normal, dev and build
        // dependencies. If it isn't, look at normal and build dependencies.
        //
        // XXX double check the assertion that Cargo doesn't consider dev-dependencies of
        // non-workspace crates.
        let unified_metadata =
            edge.normal()
                .into_iter()
                .chain(edge.build())
                .chain(if from.in_workspace() {
                    edge.dev()
                } else {
                    None
                });

        let unified_features: HashSet<&str> = unified_metadata
            .flat_map(|metadata| {
                // Packages without an explicit feature named "default" get pointed to the base.
                let default = if metadata.uses_default_features() && to.has_default_feature() {
                    Some("default")
                } else {
                    None
                };
                default
                    .into_iter()
                    .chain(metadata.features().iter().map(|s| s.as_str()))
            })
            .collect();

        // What feature unification does not impact, though, is whether the dependency is
        // actually included in the build or not. Again, consider:
        //
        // [dependencies]
        // foo = { version = "1", features = ["a"] }
        //
        // [build-dependencies]
        // foo = { version = "1", optional = true, features = ["b"] }
        //
        // This will include 'foo' as a normal dependency but *not* as a build dependency by
        // default. However, the normal dependency will include both features "a" and "b".
        //
        // This means that up to two separate edges have to be represented:
        // * a 'mandatory edge', which will be from the base node for 'from' to the feature
        //   nodes for each feature in 'to'.
        // * an 'optional edge', which will be from the feature node (from, dep_name) to the
        //   feature nodes for each feature in 'to'.

        fn extract<T: Eq>(x: Option<T>, expected_val: T, track: &mut bool) -> bool {
            match &x {
                Some(val) if val == &expected_val => {
                    *track = true;
                    true
                }
                _ => false,
            }
        }

        // None = no edge, false = mandatory, true = optional
        let normal = edge.normal().map(|metadata| metadata.optional());
        let build = edge.build().map(|metadata| metadata.optional());
        // None = no edge, () = mandatory (dev dependencies cannot be optional)
        let dev = edge.build().map(|_| ());

        // These variables track whether the edges should actually be added to the graph -- an edge
        // where everything's set to false won't be.
        let mut add_optional = false;
        let mut add_mandatory = false;

        let optional_edge = FeatureEdge::Dependency {
            normal: extract(normal, true, &mut add_optional),
            build: extract(build, true, &mut add_optional),
            dev: false,
        };
        let mandatory_edge = FeatureEdge::Dependency {
            normal: extract(normal, false, &mut add_mandatory),
            build: extract(build, false, &mut add_mandatory),
            dev: extract(dev, (), &mut add_mandatory),
        };

        if add_optional {
            // If add_optional is true, the dep name would have been added as an optional dependency
            // node.
            let from_node = FeatureNodeRef::new(from.node_idx, edge.dep_name());
            let to_nodes = FeatureNodeRef::base_and_all_features(
                to.node_idx,
                unified_features.iter().copied(),
            );
            self.add_edges(from_node, to_nodes, optional_edge, || {
                FeatureBuildStage::AddDependencyEdges {
                    package_id: from.id().clone(),
                    dep_name: edge.dep_name().to_string(),
                    optional: true,
                }
            });
        }
        if add_mandatory {
            let from_node = FeatureNodeRef::base(from.node_idx);
            let to_nodes = FeatureNodeRef::base_and_all_features(
                to.node_idx,
                unified_features.iter().copied(),
            );
            self.add_edges(from_node, to_nodes, mandatory_edge, || {
                FeatureBuildStage::AddDependencyEdges {
                    package_id: from.id().clone(),
                    dep_name: edge.dep_name().to_string(),
                    optional: false,
                }
            });
        }
    }

    fn add_node(
        &mut self,
        feature_id: FeatureNode,
        feature_type: FeatureType,
    ) -> NodeIndex<FeatureIx> {
        let feature_idx = self.graph.add_node(feature_id.clone());
        self.map.insert(
            feature_id,
            FeatureMetadata {
                feature_idx,
                feature_type,
            },
        );
        feature_idx
    }

    fn add_edges<'a>(
        &mut self,
        from_node: FeatureNodeRef<'a>,
        to_nodes: impl IntoIterator<Item = FeatureNodeRef<'a>>,
        edge: FeatureEdge,
        stage_fn: impl Fn() -> FeatureBuildStage,
    ) {
        // The from node should always be present because it is a known node.
        let from_node_idx = self.lookup_node(&from_node).unwrap_or_else(|| {
            panic!(
                "while adding feature edges, missing 'from': {:?}",
                from_node
            );
        });
        to_nodes.into_iter().for_each(|to_node| {
            match self.lookup_node(&to_node) {
                Some(to_node_idx) => {
                    self.graph
                        .update_edge(from_node_idx, to_node_idx, edge.clone());
                }
                None => {
                    // It is possible to specify a feature that doesn't actually exist, and cargo
                    // will accept that if the feature isn't resolved. One example is the cfg-if
                    // crate, where version 0.1.9 has the `rustc-dep-of-std` feature commented out,
                    // and several crates try to enable that feature:
                    // https://github.com/alexcrichton/cfg-if/issues/22
                    //
                    // Since these aren't fatal errors, it seems like the best we can do is to store
                    // such issues as warnings.
                    let feature_name = to_node
                        .feature
                        .expect("base feature should always be found")
                        .to_string();
                    self.warnings.push(FeatureGraphWarning::MissingFeature {
                        stage: stage_fn(),
                        package_id: self.package_graph.dep_graph[to_node.node_idx].clone(),
                        feature_name,
                    });
                }
            }
        })
    }

    fn lookup_node(&self, node: &dyn FeatureKey) -> Option<NodeIndex<FeatureIx>> {
        self.map.get(node).map(|metadata| metadata.feature_idx)
    }

    fn build(self) -> FeatureGraphImpl {
        FeatureGraphImpl {
            graph: self.graph,
            map: self.map,
            warnings: self.warnings,
        }
    }
}

// This trait is a fancy way to avoid memory allocations during key lookups. The code is adapted
// from http://idubrov.name/rust/2018/06/01/tricking-the-hashmap.html.
trait FeatureKey {
    fn key(&self) -> FeatureNodeRef;
}

impl FeatureKey for FeatureNode {
    fn key(&self) -> FeatureNodeRef {
        FeatureNodeRef {
            node_idx: self.node_idx,
            feature: self.feature.as_ref().map(|name| name.as_ref()),
        }
    }
}

impl<'a> FeatureKey for FeatureNodeRef<'a> {
    fn key(&self) -> FeatureNodeRef {
        FeatureNodeRef {
            node_idx: self.node_idx,
            feature: self.feature.as_ref().copied(),
        }
    }
}

impl<'a> Borrow<dyn FeatureKey + 'a> for FeatureNode {
    fn borrow(&self) -> &(dyn FeatureKey + 'a) {
        self
    }
}

impl<'a> Eq for (dyn FeatureKey + 'a) {}

impl<'a> PartialEq for (dyn FeatureKey + 'a) {
    fn eq(&self, other: &dyn FeatureKey) -> bool {
        self.key() == other.key()
    }
}

impl<'a> Hash for (dyn FeatureKey + 'a) {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key().hash(state)
    }
}
