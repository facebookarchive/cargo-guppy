// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{
    errors::{FeatureBuildStage, FeatureGraphWarning},
    graph::{
        feature::{
            ConditionalLinkImpl, FeatureEdge, FeatureGraphImpl, FeatureMetadataImpl, FeatureNode,
            FeatureType,
        },
        DepRequiredOrOptional, DependencyReq, FeatureIx, PackageGraph, PackageIx, PackageLink,
        PackageMetadata,
    },
    platform::PlatformStatusImpl,
};
use cargo_metadata::DependencyKind;
use once_cell::sync::OnceCell;
use petgraph::prelude::*;
use std::{collections::HashMap, iter};

#[derive(Debug)]
pub(super) struct FeatureGraphBuildState {
    graph: Graph<FeatureNode, FeatureEdge, Directed, FeatureIx>,
    // Map from package ixs to the base (first) feature for each package.
    base_ixs: Vec<NodeIndex<FeatureIx>>,
    map: HashMap<FeatureNode, FeatureMetadataImpl>,
    warnings: Vec<FeatureGraphWarning>,
}

impl FeatureGraphBuildState {
    pub(super) fn new(package_graph: &PackageGraph) -> Self {
        let package_count = package_graph.package_count();
        Self {
            // Each package corresponds to at least one feature ID.
            graph: Graph::with_capacity(package_count, package_count),
            // Each package corresponds to exactly one base feature ix, and there's one last ix at
            // the end.
            base_ixs: Vec::with_capacity(package_count + 1),
            map: HashMap::with_capacity(package_count),
            warnings: vec![],
        }
    }

    /// Add nodes for every feature in this package + the base package, and add edges from every
    /// feature to the base package.
    pub(super) fn add_nodes(&mut self, package: PackageMetadata<'_>) {
        let base_node = FeatureNode::base(package.package_ix());
        let base_ix = self.add_node(base_node, FeatureType::BasePackage);
        self.base_ixs.push(base_ix);
        FeatureNode::named_features(package).for_each(|feature_node| {
            let feature_ix = self.add_node(feature_node, FeatureType::NamedFeature);
            self.graph
                .update_edge(feature_ix, base_ix, FeatureEdge::FeatureToBase);
        });

        package.optional_deps_full().for_each(|(n, _)| {
            let dep_idx = self.add_node(
                FeatureNode::new(package.package_ix(), n),
                FeatureType::OptionalDep,
            );
            self.graph
                .update_edge(dep_idx, base_ix, FeatureEdge::FeatureToBase);
        });
    }

    /// Mark the end of adding nodes.
    pub(super) fn end_nodes(&mut self) {
        self.base_ixs.push(NodeIndex::new(self.graph.node_count()));
    }

    pub(super) fn add_named_feature_edges(&mut self, metadata: PackageMetadata<'_>) {
        let dep_name_to_link: HashMap<_, _> = metadata
            .direct_links()
            .map(|link| (link.dep_name(), link))
            .collect();

        metadata
            .named_features_full()
            .for_each(|(n, from_feature, feature_deps)| {
                let from_node = FeatureNode::new(metadata.package_ix(), n);
                let to_nodes_edges: Vec<_> = feature_deps
                    .iter()
                    .flat_map(|feature_dep| {
                        let (dep_name, to_feature) = Self::split_feature_dep(feature_dep);
                        let (cross_node_edge, same_node_edge) = match dep_name {
                            Some(dep_name) => {
                                if let Some(link) = dep_name_to_link.get(dep_name) {
                                    // dependency from (`main`, `a`) to (`dep, `foo`)
                                    let cross_node_edge = self
                                        .make_named_feature_node(
                                            &metadata,
                                            from_feature,
                                            &link.to(),
                                            to_feature,
                                            true,
                                        )
                                        .map(|cross_node| {
                                            // This is a cross-package link. The platform-specific
                                            // requirements still apply, so grab them from the
                                            // PackageLink.
                                            (cross_node, Self::make_named_feature_cross_edge(link))
                                        });

                                    // If the package is present as an optional dependency, it is
                                    // implicitly activated by the feature:
                                    // from (`main`, `a`) to (`main`, `dep`)
                                    let same_node_edge = self
                                        .make_named_feature_node(
                                            &metadata,
                                            from_feature,
                                            &metadata,
                                            dep_name,
                                            // Don't warn if this dep isn't optional.
                                            false,
                                        )
                                        .map(|same_node| {
                                            (same_node, Self::make_named_feature_cross_edge(link))
                                        });
                                    (cross_node_edge, same_node_edge)
                                } else {
                                    // The destination package was unknown to the graph.
                                    // XXX may need to be revisited if we start modeling unresolved
                                    // dependencies.
                                    (None, None)
                                }
                            }
                            None => {
                                let same_node_edge = self
                                    .make_named_feature_node(
                                        &metadata,
                                        from_feature,
                                        &metadata,
                                        to_feature,
                                        true,
                                    )
                                    .map(|same_node| (same_node, FeatureEdge::FeatureDependency));
                                (None, same_node_edge)
                            }
                        };

                        cross_node_edge.into_iter().chain(same_node_edge)
                    })
                    // The flat_map above holds an &mut reference to self, which is why it needs to
                    // be collected.
                    .collect();

                // Don't create a map to the base 'from' node since it is already created in
                // add_nodes.
                self.add_edges(from_node, to_nodes_edges);
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

    fn make_named_feature_node(
        &mut self,
        from_package: &PackageMetadata<'_>,
        from_feature: &str,
        to_package: &PackageMetadata<'_>,
        to_feature: &str,
        warn: bool,
    ) -> Option<FeatureNode> {
        match to_package.get_feature_idx(to_feature) {
            Some(idx) => Some(FeatureNode::new(to_package.package_ix(), idx)),
            None => {
                // It is possible to specify a feature that doesn't actually exist, and cargo will
                // accept that if the feature isn't resolved. One example is the cfg-if crate, where
                // version 0.1.9 has the `rustc-dep-of-std` feature commented out, and several
                // crates try to enable that feature:
                // https://github.com/alexcrichton/cfg-if/issues/22
                //
                // Since these aren't fatal errors, it seems like the best we can do is to store
                // such issues as warnings.
                if warn {
                    self.warnings.push(FeatureGraphWarning::MissingFeature {
                        stage: FeatureBuildStage::AddNamedFeatureEdges {
                            package_id: from_package.id().clone(),
                            from_feature: from_feature.to_string(),
                        },
                        package_id: to_package.id().clone(),
                        feature_name: to_feature.to_string(),
                    });
                }
                None
            }
        }
    }

    /// Creates the cross link for situations like:
    ///
    /// ```toml
    /// [features]
    /// a = ["dep/foo"]
    /// ```
    ///
    /// (a link (`from`, `a`) to (`dep`, `foo`) is created.
    ///
    /// If `dep` is optional, the edge (`from`, `a`) to (`from`, `dep`) is also a `CrossPackage`
    /// edge.
    fn make_named_feature_cross_edge(link: &PackageLink<'_>) -> FeatureEdge {
        // This edge is enabled if the feature is enabled, which means the union of (required,
        // optional) build conditions.
        fn combine_req_opt(req: DependencyReq<'_>) -> PlatformStatusImpl {
            let mut required = req.inner.required.build_if.clone();
            required.extend(&req.inner.optional.build_if);
            required
        }

        FeatureEdge::Conditional(ConditionalLinkImpl {
            package_edge_ix: link.edge_ix(),
            normal: combine_req_opt(link.normal()),
            build: combine_req_opt(link.build()),
            dev: combine_req_opt(link.dev()),
        })
    }

    pub(super) fn add_dependency_edges(&mut self, link: PackageLink<'_>) {
        let from = link.from();

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
        // Also, feature unification is impacted by whether the dependency is optional.
        //
        // [dependencies]
        // foo = { version = "1", features = ["a"] }
        //
        // [build-dependencies]
        // foo = { version = "1", optional = true, features = ["b"] }
        //
        // This will include 'foo' as a normal dependency but *not* as a build dependency by
        // default.
        // * Without '--features foo', the `foo` dependency will be built with "a".
        // * With '--features foo', `foo` will be both a normal and a build dependency, with
        //   features "a" and "b" in both instances.
        //
        // This means that up to two separate edges have to be represented:
        // * a 'required edge', which will be from the base node for 'from' to the feature nodes
        //   for each required feature in 'to'.
        // * an 'optional edge', which will be from the feature node (from, dep_name) to the
        //   feature nodes for each optional feature in 'to'. This edge is only added if at least
        //   one line is optional.

        let unified_metadata = iter::once((DependencyKind::Normal, link.normal()))
            .chain(iter::once((DependencyKind::Build, link.build())))
            .chain(iter::once((DependencyKind::Development, link.dev())));

        let mut required_req = FeatureReq::new(link);
        let mut optional_req = FeatureReq::new(link);
        for (kind, dependency_req) in unified_metadata {
            required_req.add_features(kind, &dependency_req.inner.required, &mut self.warnings);
            optional_req.add_features(kind, &dependency_req.inner.optional, &mut self.warnings);
        }

        // Add the required edges (base -> features).
        self.add_edges(FeatureNode::base(from.package_ix()), required_req.finish());

        if !optional_req.is_empty() {
            // This means that there is at least one instance of this dependency with optional =
            // true. The dep name should have been added as an optional dependency node to the
            // package metadata.
            let from_node = FeatureNode::new(
                from.package_ix(),
                from.get_feature_idx(link.dep_name()).unwrap_or_else(|| {
                    panic!(
                        "while adding feature edges, for package '{}', optional dep '{}' missing",
                        from.id(),
                        link.dep_name(),
                    );
                }),
            );
            self.add_edges(from_node, optional_req.finish());
        }
    }

    fn add_node(
        &mut self,
        feature_id: FeatureNode,
        feature_type: FeatureType,
    ) -> NodeIndex<FeatureIx> {
        let feature_ix = self.graph.add_node(feature_id);
        self.map.insert(
            feature_id,
            FeatureMetadataImpl {
                feature_ix,
                feature_type,
            },
        );
        feature_ix
    }

    fn add_edges(
        &mut self,
        from_node: FeatureNode,
        to_nodes_edges: impl IntoIterator<Item = (FeatureNode, FeatureEdge)>,
    ) {
        // The from node should always be present because it is a known node.
        let from_ix = self.lookup_node(&from_node).unwrap_or_else(|| {
            panic!(
                "while adding feature edges, missing 'from': {:?}",
                from_node
            );
        });
        to_nodes_edges.into_iter().for_each(|(to_node, edge)| {
            let to_ix = self.lookup_node(&to_node).unwrap_or_else(|| {
                panic!("while adding feature edges, missing 'to': {:?}", to_node)
            });

            match self.graph.find_edge(from_ix, to_ix) {
                Some(edge_ix) => {
                    // The edge already exists. This could be an upgrade from a cross link to a
                    // feature dependency, for example:
                    //
                    // [package]
                    // name = "main"
                    //
                    // [dependencies]
                    // dep = { ..., optional = true }
                    //
                    // [features]
                    // "feat" = ["dep/feat", "dep"]
                    //
                    // "dep/feat" causes a cross link to be created from "main/feat" to "main/dep".
                    // However, the "dep" encountered later upgrades this link to a feature
                    // dependency.
                    let old_edge = self
                        .graph
                        .edge_weight_mut(edge_ix)
                        .expect("this edge was just found");
                    #[allow(clippy::single_match)]
                    match (old_edge, edge) {
                        (
                            old_edge @ FeatureEdge::Conditional(_),
                            edge @ FeatureEdge::FeatureDependency,
                        ) => {
                            // Upgrade this edge.
                            *old_edge = edge;
                        }
                        _ => {
                            // In all other cases, leave the old edge alone.
                        }
                    }
                }
                None => {
                    self.graph.add_edge(from_ix, to_ix, edge);
                }
            }
        })
    }

    fn lookup_node(&self, node: &FeatureNode) -> Option<NodeIndex<FeatureIx>> {
        self.map.get(node).map(|metadata| metadata.feature_ix)
    }

    pub(super) fn build(self) -> FeatureGraphImpl {
        FeatureGraphImpl {
            graph: self.graph,
            base_ixs: self.base_ixs,
            map: self.map,
            warnings: self.warnings,
            sccs: OnceCell::new(),
        }
    }
}

#[derive(Debug)]
struct FeatureReq<'g> {
    link: PackageLink<'g>,
    to: PackageMetadata<'g>,
    edge_ix: EdgeIndex<PackageIx>,
    to_default_idx: Option<usize>,
    // This will contain any build states that aren't empty.
    features: HashMap<Option<usize>, DependencyBuildState>,
}

impl<'g> FeatureReq<'g> {
    fn new(link: PackageLink<'g>) -> Self {
        let to = link.to();
        Self {
            link,
            to,
            edge_ix: link.edge_ix(),
            to_default_idx: to.get_feature_idx("default"),
            features: HashMap::new(),
        }
    }

    fn is_empty(&self) -> bool {
        // self.features only consists of non-empty build states.
        self.features.is_empty()
    }

    fn add_features(
        &mut self,
        dep_kind: DependencyKind,
        req: &DepRequiredOrOptional,
        warnings: &mut Vec<FeatureGraphWarning>,
    ) {
        // Base feature.
        self.extend(None, dep_kind, &req.build_if);
        // Default feature (or base if it isn't present).
        self.extend(self.to_default_idx, dep_kind, &req.default_features_if);

        for (feature, status) in &req.feature_targets {
            match self.to.get_feature_idx(feature) {
                Some(feature_idx) => {
                    self.extend(Some(feature_idx), dep_kind, status);
                }
                None => {
                    // The destination feature is missing -- this is accepted by cargo
                    // in some circumstances, so use a warning rather than an error.
                    warnings.push(FeatureGraphWarning::MissingFeature {
                        stage: FeatureBuildStage::AddDependencyEdges {
                            package_id: self.link.from().id().clone(),
                            dep_name: self.link.dep_name().to_string(),
                        },
                        package_id: self.to.id().clone(),
                        feature_name: feature.to_string(),
                    });
                }
            }
        }
    }

    fn extend(
        &mut self,
        feature_idx: Option<usize>,
        dep_kind: DependencyKind,
        status: &PlatformStatusImpl,
    ) {
        let package_edge_ix = self.edge_ix;
        if !status.is_never() {
            self.features
                .entry(feature_idx)
                .or_insert_with(|| DependencyBuildState::new(package_edge_ix))
                .extend(dep_kind, status);
        }
    }

    fn finish(self) -> impl Iterator<Item = (FeatureNode, FeatureEdge)> {
        let package_ix = self.to.package_ix();
        self.features
            .into_iter()
            .map(move |(feature_idx, build_state)| {
                // extend ensures that the build states aren't empty. Double-check that.
                debug_assert!(!build_state.is_empty(), "build states are always non-empty");
                (
                    FeatureNode::new_opt(package_ix, feature_idx),
                    build_state.finish(),
                )
            })
    }
}

#[derive(Debug)]
struct DependencyBuildState {
    package_edge_ix: EdgeIndex<PackageIx>,
    normal: PlatformStatusImpl,
    build: PlatformStatusImpl,
    dev: PlatformStatusImpl,
}

impl DependencyBuildState {
    fn new(package_edge_ix: EdgeIndex<PackageIx>) -> Self {
        Self {
            package_edge_ix,
            normal: PlatformStatusImpl::default(),
            build: PlatformStatusImpl::default(),
            dev: PlatformStatusImpl::default(),
        }
    }

    fn extend(&mut self, dep_kind: DependencyKind, status: &PlatformStatusImpl) {
        match dep_kind {
            DependencyKind::Normal => self.normal.extend(status),
            DependencyKind::Build => self.build.extend(status),
            DependencyKind::Development => self.dev.extend(status),
            _ => panic!("unknown dependency kind"),
        }
    }

    fn is_empty(&self) -> bool {
        self.normal.is_never() && self.build.is_never() && self.dev.is_never()
    }

    fn finish(self) -> FeatureEdge {
        FeatureEdge::Conditional(ConditionalLinkImpl {
            package_edge_ix: self.package_edge_ix,
            normal: self.normal,
            build: self.build,
            dev: self.dev,
        })
    }
}
