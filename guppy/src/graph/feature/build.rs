// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::errors::{FeatureBuildStage, FeatureGraphWarning};
use crate::graph::feature::{
    FeatureEdge, FeatureGraphImpl, FeatureMetadataImpl, FeatureNode, FeatureType,
};
use crate::graph::{DependencyLink, FeatureIx, PackageGraph, PackageMetadata};
use petgraph::prelude::*;
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub(super) struct FeatureGraphBuildState<'g> {
    package_graph: &'g PackageGraph,
    graph: Graph<FeatureNode, FeatureEdge, Directed, FeatureIx>,
    map: HashMap<FeatureNode, FeatureMetadataImpl>,
    warnings: Vec<FeatureGraphWarning>,
}

impl<'g> FeatureGraphBuildState<'g> {
    pub(super) fn new(package_graph: &'g PackageGraph) -> Self {
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
    pub(super) fn add_nodes(&mut self, package: &'g PackageMetadata) {
        let base_node = FeatureNode::base(package.package_ix);
        let base_idx = self.add_node(base_node, FeatureType::BasePackage);
        FeatureNode::named_features(package).for_each(|feature_node| {
            let feature_ix = self.add_node(feature_node, FeatureType::NamedFeature);
            self.graph
                .update_edge(feature_ix, base_idx, FeatureEdge::FeatureToBase);
        });

        package.optional_deps_full().for_each(|(n, _)| {
            let dep_idx = self.add_node(
                FeatureNode::new(package.package_ix, n),
                FeatureType::OptionalDep,
            );
            self.graph
                .update_edge(dep_idx, base_idx, FeatureEdge::FeatureToBase);
        });
    }

    pub(super) fn add_named_feature_edges(&mut self, metadata: &PackageMetadata) {
        let dep_name_to_metadata: HashMap<_, _> = self
            .package_graph
            .dep_links(metadata.id())
            .expect("valid metadata")
            .map(|link| (link.edge.dep_name(), link.to))
            .collect();

        metadata
            .named_features_full()
            .for_each(|(n, named_feature, feature_deps)| {
                let from_node = FeatureNode::new(metadata.package_ix, n);
                let to_nodes: Vec<_> = feature_deps
                    .iter()
                    .filter_map(|feature_dep| {
                        let (dep_name, to_feature_name) = Self::split_feature_dep(feature_dep);
                        match dep_name {
                            Some(dep_name) => {
                                match dep_name_to_metadata.get(dep_name) {
                                    Some(to_metadata) => {
                                        match to_metadata.get_feature_idx(to_feature_name) {
                                            Some(to_feature_idx) => Some(FeatureNode::new(
                                                to_metadata.package_ix,
                                                to_feature_idx,
                                            )),
                                            None => {
                                                // It is possible to specify a feature that doesn't
                                                // actually exist, and cargo will accept that if the
                                                // feature isn't resolved. One example is the cfg-if
                                                // crate, where version 0.1.9 has the
                                                // `rustc-dep-of-std` feature commented out, and
                                                // several crates try to enable that feature:
                                                // https://github.com/alexcrichton/cfg-if/issues/22
                                                //
                                                // Since these aren't fatal errors, it seems like
                                                // the best we can do is to store such issues as
                                                // warnings.
                                                self.warnings
                                                    .push(FeatureGraphWarning::MissingFeature {
                                                    stage:
                                                        FeatureBuildStage::AddNamedFeatureEdges {
                                                            package_id: metadata.id().clone(),
                                                            from_feature: named_feature.to_string(),
                                                        },
                                                    package_id: to_metadata.id().clone(),
                                                    feature_name: to_feature_name.to_string(),
                                                });
                                                None
                                            }
                                        }
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
                            None => {
                                match metadata.get_feature_idx(to_feature_name) {
                                    Some(to_feature_idx) => {
                                        Some(FeatureNode::new(metadata.package_ix, to_feature_idx))
                                    }
                                    None => {
                                        // See blurb above, though maybe this should be tightened a
                                        // bit (errors and not warning?)
                                        self.warnings.push(FeatureGraphWarning::MissingFeature {
                                            stage: FeatureBuildStage::AddNamedFeatureEdges {
                                                package_id: metadata.id().clone(),
                                                from_feature: named_feature.to_string(),
                                            },
                                            package_id: metadata.id().clone(),
                                            feature_name: to_feature_name.to_string(),
                                        });
                                        None
                                    }
                                }
                            }
                        }
                    })
                    .collect();
                // Don't create a map to the base 'from' node since it is already created in
                // add_nodes.
                self.add_edges(from_node, to_nodes, FeatureEdge::FeatureDependency);
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

    pub(super) fn add_dependency_edges(&mut self, link: DependencyLink<'_>) {
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

        let mut unified_features = HashSet::new();
        for metadata in unified_metadata {
            let default_idx = match (
                metadata.uses_default_features(),
                to.get_feature_idx("default"),
            ) {
                (true, Some(default_idx)) => Some(default_idx),
                // Packages without an explicit feature named "default" get pointed to the base.
                _ => None,
            };

            let feature_idxs =
                default_idx
                    .into_iter()
                    .chain(metadata.features().iter().filter_map(|to_feature| {
                        match to.get_feature_idx(to_feature) {
                            Some(feature_idx) => Some(feature_idx),
                            None => {
                                // The destination feature is missing -- this is accepted by cargo
                                // in some circumstances, so use a warning rather than an error.
                                self.warnings.push(FeatureGraphWarning::MissingFeature {
                                    stage: FeatureBuildStage::AddDependencyEdges {
                                        package_id: from.id().clone(),
                                        dep_name: edge.dep_name().to_string(),
                                    },
                                    package_id: to.id().clone(),
                                    feature_name: to_feature.clone(),
                                });
                                None
                            }
                        }
                    }));
            unified_features.extend(feature_idxs);
        }

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
        let dev = edge.dev().map(|_| ());

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
            // node to the package metadata.
            let from_node = FeatureNode::new(
                from.package_ix,
                from.get_feature_idx(edge.dep_name()).unwrap_or_else(|| {
                    panic!(
                        "while adding feature edges, for package '{}', optional dep '{}' missing",
                        from.id(),
                        edge.dep_name(),
                    );
                }),
            );
            let to_nodes =
                FeatureNode::base_and_all_features(to.package_ix, unified_features.iter().copied());
            self.add_edges(from_node, to_nodes, optional_edge);
        }
        if add_mandatory {
            let from_node = FeatureNode::base(from.package_ix);
            let to_nodes =
                FeatureNode::base_and_all_features(to.package_ix, unified_features.iter().copied());
            self.add_edges(from_node, to_nodes, mandatory_edge);
        }
    }

    fn add_node(
        &mut self,
        feature_id: FeatureNode,
        feature_type: FeatureType,
    ) -> NodeIndex<FeatureIx> {
        let feature_ix = self.graph.add_node(feature_id.clone());
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
        to_nodes: impl IntoIterator<Item = FeatureNode>,
        edge: FeatureEdge,
    ) {
        // The from node should always be present because it is a known node.
        let from_ix = self.lookup_node(&from_node).unwrap_or_else(|| {
            panic!(
                "while adding feature edges, missing 'from': {:?}",
                from_node
            );
        });
        to_nodes.into_iter().for_each(|to_node| {
            let to_ix = self.lookup_node(&to_node).unwrap_or_else(|| {
                panic!("while adding feature edges, missing 'to': {:?}", to_node)
            });
            self.graph.update_edge(from_ix, to_ix, edge.clone());
        })
    }

    fn lookup_node(&self, node: &FeatureNode) -> Option<NodeIndex<FeatureIx>> {
        self.map.get(node).map(|metadata| metadata.feature_ix)
    }

    pub(super) fn build(self) -> FeatureGraphImpl {
        FeatureGraphImpl {
            graph: self.graph,
            map: self.map,
            warnings: self.warnings,
        }
    }
}
