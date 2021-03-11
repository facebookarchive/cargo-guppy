// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{
    dep_helpers::{
        assert_all_links, assert_deps_internal, assert_topo_ids, assert_topo_metadatas,
        assert_transitive_deps_internal,
    },
    package_id,
};
use camino::Utf8PathBuf;
use guppy::{
    errors::FeatureGraphWarning,
    graph::{
        BuildTargetId, BuildTargetKind, DependencyDirection, EnabledStatus, EnabledTernary,
        PackageGraph, PackageLink, PackageMetadata, PackageSource, Workspace,
    },
    DependencyKind, PackageId, Platform, Version,
};
use pretty_assertions::assert_eq;
use std::collections::{BTreeMap, HashMap};

/// This captures metadata fields that are relevant for tests. They are meant to be written out
/// lazily as tests are filled out -- feel free to add more details as necessary!
pub struct FixtureDetails {
    workspace_members: Option<BTreeMap<Utf8PathBuf, PackageId>>,
    package_details: HashMap<PackageId, PackageDetails>,
    link_details: HashMap<(PackageId, PackageId), LinkDetails>,
    feature_graph_warnings: Vec<FeatureGraphWarning>,
    cycles: Vec<Vec<PackageId>>,
}

impl FixtureDetails {
    pub fn new(package_details: HashMap<PackageId, PackageDetails>) -> Self {
        Self {
            workspace_members: None,
            package_details,
            link_details: HashMap::new(),
            feature_graph_warnings: vec![],
            cycles: vec![],
        }
    }

    pub fn with_workspace_members<'a>(
        mut self,
        workspace_members: impl IntoIterator<Item = (impl Into<Utf8PathBuf>, &'a str)>,
    ) -> Self {
        self.workspace_members = Some(
            workspace_members
                .into_iter()
                .map(|(path, id)| (path.into(), package_id(id)))
                .collect(),
        );
        self
    }

    pub fn with_link_details(
        mut self,
        link_details: HashMap<(PackageId, PackageId), LinkDetails>,
    ) -> Self {
        self.link_details = link_details;
        self
    }

    pub fn with_feature_graph_warnings(mut self, mut warnings: Vec<FeatureGraphWarning>) -> Self {
        warnings.sort();
        self.feature_graph_warnings = warnings;
        self
    }

    pub fn with_cycles(mut self, cycles: Vec<Vec<&'static str>>) -> Self {
        let cycles: Vec<_> = cycles
            .into_iter()
            .map(|cycle| cycle.into_iter().map(package_id).collect())
            .collect();
        // Don't sort because the order returned by all_cycles (both the outer and inner vecs) is
        // significant.
        self.cycles = cycles;
        self
    }

    pub fn known_ids(&self) -> impl Iterator<Item = &PackageId> {
        self.package_details.keys()
    }

    pub fn assert_workspace(&self, workspace: Workspace) {
        if let Some(expected_members) = &self.workspace_members {
            let members: Vec<_> = workspace
                .iter_by_path()
                .map(|(path, metadata)| (path, metadata.id()))
                .collect();
            assert_eq!(
                expected_members
                    .iter()
                    .map(|(path, id)| (path.as_path(), id))
                    .collect::<Vec<_>>(),
                members,
                "workspace members should be correct"
            );

            assert_eq!(
                workspace.iter_by_path().len(),
                workspace.iter_by_name().len(),
                "workspace.members() and members_by_name() return the same number of items"
            );
            for (name, metadata) in workspace.iter_by_name() {
                assert_eq!(
                    name,
                    metadata.name(),
                    "members_by_name returns consistent results"
                );
            }
        }
    }

    pub fn assert_topo(&self, graph: &PackageGraph) {
        assert_topo_ids(graph, DependencyDirection::Forward, "topo sort");
        assert_topo_ids(graph, DependencyDirection::Reverse, "reverse topo sort");
        assert_topo_metadatas(graph, DependencyDirection::Forward, "topo sort (metadatas)");
        assert_topo_metadatas(
            graph,
            DependencyDirection::Reverse,
            "reverse topo sort (metadatas)",
        );
        assert_all_links(graph, DependencyDirection::Forward, "all links");
        assert_all_links(graph, DependencyDirection::Reverse, "all links reversed");
    }

    pub fn assert_metadata(&self, id: &PackageId, metadata: PackageMetadata<'_>, msg: &str) {
        let details = &self.package_details[id];
        details.assert_metadata(metadata, msg);
    }

    // ---
    // Build targets
    // ---

    pub fn has_build_targets(&self, id: &PackageId) -> bool {
        let details = &self.package_details[id];
        details.build_targets.is_some()
    }

    pub fn assert_build_targets(&self, metadata: PackageMetadata<'_>, msg: &str) {
        let build_targets = self.package_details[metadata.id()]
            .build_targets
            .as_ref()
            .unwrap();

        let mut actual: Vec<_> = metadata
            .build_targets()
            .map(|build_target| {
                // Strip off the manifest path from the beginning.
                let path = build_target
                    .path()
                    .strip_prefix(
                        metadata
                            .manifest_path()
                            .parent()
                            .expect("manifest path is a file"),
                    )
                    .expect("build target path is inside source dir")
                    .to_path_buf();

                (build_target.id(), build_target.kind().clone(), path)
            })
            .collect();
        actual.sort();

        assert_eq!(build_targets, &actual, "{}: build targets match", msg,);
    }

    // ---
    // Direct dependencies
    // ---

    /// Returns true if the deps for this package are available to test against.
    pub fn has_deps(&self, id: &PackageId) -> bool {
        let details = &self.package_details[id];
        details.deps.is_some()
    }

    pub fn assert_deps(&self, graph: &PackageGraph, id: &PackageId, msg: &str) {
        let details = &self.package_details[id];
        assert_deps_internal(&graph, DependencyDirection::Forward, details, msg);
    }

    /// Returns true if the reverse deps for this package are available to test against.
    pub fn has_reverse_deps(&self, id: &PackageId) -> bool {
        let details = &self.package_details[id];
        details.reverse_deps.is_some()
    }

    pub fn assert_reverse_deps(&self, graph: &PackageGraph, id: &PackageId, msg: &str) {
        let details = &self.package_details[id];
        assert_deps_internal(&graph, DependencyDirection::Reverse, details, msg);
    }

    // ---
    // Transitive dependencies
    // ---

    /// Returns true if the transitive deps for this package are available to test against.
    pub fn has_transitive_deps(&self, id: &PackageId) -> bool {
        let details = &self.package_details[id];
        details.transitive_deps.is_some()
    }

    pub fn assert_transitive_deps(&self, graph: &PackageGraph, id: &PackageId, msg: &str) {
        assert_transitive_deps_internal(
            graph,
            DependencyDirection::Forward,
            &self.package_details[id],
            msg,
        )
    }

    /// Returns true if the transitive reverse deps for this package are available to test against.
    pub fn has_transitive_reverse_deps(&self, id: &PackageId) -> bool {
        let details = &self.package_details[id];
        details.transitive_reverse_deps.is_some()
    }

    pub fn assert_transitive_reverse_deps(&self, graph: &PackageGraph, id: &PackageId, msg: &str) {
        assert_transitive_deps_internal(
            graph,
            DependencyDirection::Reverse,
            &self.package_details[id],
            msg,
        )
    }

    // ---
    // Links
    // ---

    pub fn assert_link_details(&self, graph: &PackageGraph, msg: &str) {
        for ((from, to), details) in &self.link_details {
            let metadata = graph
                .metadata(from)
                .unwrap_or_else(|err| panic!("{}: {}", msg, err));
            let mut links: Vec<_> = metadata
                .direct_links()
                .filter(|link| link.to().id() == to)
                .collect();
            assert_eq!(
                links.len(),
                1,
                "{}: exactly 1 link between '{}' and '{}'",
                msg,
                from,
                to
            );

            let link = links.pop().unwrap();
            let msg = format!("{}: {} -> {}", msg, from, to);
            details.assert_metadata(link, &msg);
        }
    }

    // ---
    // Features
    // ---

    pub fn has_named_features(&self, id: &PackageId) -> bool {
        self.package_details[id].named_features.is_some()
    }

    pub fn assert_named_features(&self, graph: &PackageGraph, id: &PackageId, msg: &str) {
        let mut actual: Vec<_> = graph
            .metadata(id)
            .expect("package id should be valid")
            .named_features()
            .collect();
        actual.sort_unstable();
        let expected = self.package_details[id].named_features.as_ref().unwrap();
        assert_eq!(expected, &actual, "{}", msg);
    }

    pub fn assert_feature_graph_warnings(&self, graph: &PackageGraph, msg: &str) {
        let mut actual: Vec<_> = graph.feature_graph().build_warnings().to_vec();
        actual.sort();
        assert_eq!(&self.feature_graph_warnings, &actual, "{}", msg);
    }

    // ---
    // Cycles
    // ---

    pub fn assert_cycles(&self, graph: &PackageGraph, msg: &str) {
        let actual: Vec<_> = graph.cycles().all_cycles().collect();
        // Don't sort because the order returned by all_cycles (both the outer and inner vecs) is
        // significant.
        assert_eq!(&self.cycles, &actual, "{}", msg);
    }
}

pub struct PackageDetails {
    id: PackageId,
    name: &'static str,
    version: Version,
    authors: Vec<&'static str>,
    description: Option<&'static str>,
    license: Option<&'static str>,

    source: Option<PackageSource<'static>>,
    build_targets: Option<
        Vec<(
            BuildTargetId<'static>,
            BuildTargetKind<'static>,
            Utf8PathBuf,
        )>,
    >,
    // The vector items are (name, package id).
    // XXX add more details about dependency edges here?
    deps: Option<Vec<(&'static str, PackageId)>>,
    reverse_deps: Option<Vec<(&'static str, PackageId)>>,
    transitive_deps: Option<Vec<PackageId>>,
    transitive_reverse_deps: Option<Vec<PackageId>>,
    named_features: Option<Vec<&'static str>>,
}

impl PackageDetails {
    pub fn new(
        id: &'static str,
        name: &'static str,
        version: &'static str,
        authors: Vec<&'static str>,
        description: Option<&'static str>,
        license: Option<&'static str>,
    ) -> Self {
        Self {
            id: package_id(id),
            name,
            version: Version::parse(version).expect("version should be valid"),
            authors,
            description,
            license,
            source: None,
            build_targets: None,
            deps: None,
            reverse_deps: None,
            transitive_deps: None,
            transitive_reverse_deps: None,
            named_features: None,
        }
    }

    pub fn with_workspace_path(mut self, path: &'static str) -> Self {
        self.source = Some(PackageSource::Workspace(path.into()));
        self
    }

    pub fn with_local_path(mut self, path: &'static str) -> Self {
        self.source = Some(PackageSource::Path(path.into()));
        self
    }

    pub fn with_crates_io(self) -> Self {
        self.with_external_source(PackageSource::CRATES_IO_REGISTRY)
    }

    pub fn with_external_source(mut self, source: &'static str) -> Self {
        self.source = Some(PackageSource::External(source));
        self
    }

    pub fn with_build_targets(
        mut self,
        mut build_targets: Vec<(
            BuildTargetId<'static>,
            BuildTargetKind<'static>,
            &'static str,
        )>,
    ) -> Self {
        build_targets.sort();
        self.build_targets = Some(
            build_targets
                .into_iter()
                .map(|(id, kind, path)| (id, kind, path.to_string().into()))
                .collect(),
        );
        self
    }

    pub fn with_deps(mut self, mut deps: Vec<(&'static str, &'static str)>) -> Self {
        deps.sort_unstable();
        self.deps = Some(
            deps.into_iter()
                .map(|(name, id)| (name, package_id(id)))
                .collect(),
        );
        self
    }

    pub fn with_reverse_deps(
        mut self,
        mut reverse_deps: Vec<(&'static str, &'static str)>,
    ) -> Self {
        reverse_deps.sort_unstable();
        self.reverse_deps = Some(
            reverse_deps
                .into_iter()
                .map(|(name, id)| (name, package_id(id)))
                .collect(),
        );
        self
    }

    pub fn with_transitive_deps(mut self, mut transitive_deps: Vec<&'static str>) -> Self {
        transitive_deps.sort_unstable();
        self.transitive_deps = Some(transitive_deps.into_iter().map(package_id).collect());
        self
    }

    pub fn with_transitive_reverse_deps(
        mut self,
        mut transitive_reverse_deps: Vec<&'static str>,
    ) -> Self {
        transitive_reverse_deps.sort_unstable();
        self.transitive_reverse_deps = Some(
            transitive_reverse_deps
                .into_iter()
                .map(package_id)
                .collect(),
        );
        self
    }

    pub fn with_named_features(mut self, mut named_features: Vec<&'static str>) -> Self {
        named_features.sort_unstable();
        self.named_features = Some(named_features);
        self
    }

    pub fn insert_into(self, map: &mut HashMap<PackageId, PackageDetails>) {
        map.insert(self.id.clone(), self);
    }

    pub fn id(&self) -> &PackageId {
        &self.id
    }

    pub fn deps(&self, direction: DependencyDirection) -> Option<&[(&'static str, PackageId)]> {
        match direction {
            DependencyDirection::Forward => self.deps.as_deref(),
            DependencyDirection::Reverse => self.reverse_deps.as_deref(),
        }
    }

    pub fn transitive_deps(&self, direction: DependencyDirection) -> Option<&[PackageId]> {
        match direction {
            DependencyDirection::Forward => self.transitive_deps.as_deref(),
            DependencyDirection::Reverse => self.transitive_reverse_deps.as_deref(),
        }
    }

    pub fn assert_metadata(&self, metadata: PackageMetadata<'_>, msg: &str) {
        assert_eq!(&self.id, metadata.id(), "{}: same package ID", msg);
        assert_eq!(self.name, metadata.name(), "{}: same name", msg);
        assert_eq!(&self.version, metadata.version(), "{}: same version", msg);
        assert_eq!(
            &self.authors,
            &metadata
                .authors()
                .iter()
                .map(|author| author.as_str())
                .collect::<Vec<_>>(),
            "{}: same authors",
            msg
        );
        assert_eq!(
            &self.description,
            &metadata.description(),
            "{}: same description",
            msg
        );
        assert_eq!(&self.license, &metadata.license(), "{}: same license", msg);
        if let Some(source) = &self.source {
            assert_eq!(source, &metadata.source(), "{}: same source", msg);
        }
    }
}

#[derive(Clone, Debug)]
pub struct LinkDetails {
    from: PackageId,
    to: PackageId,
    platform_results: Vec<(DependencyKind, Platform<'static>, PlatformResults)>,
    features: Vec<(DependencyKind, Vec<&'static str>)>,
}

impl LinkDetails {
    pub fn new(from: PackageId, to: PackageId) -> Self {
        Self {
            from,
            to,
            platform_results: vec![],
            features: vec![],
        }
    }

    pub fn with_platform_status(
        mut self,
        dep_kind: DependencyKind,
        platform: Platform<'static>,
        status: PlatformResults,
    ) -> Self {
        self.platform_results.push((dep_kind, platform, status));
        self
    }

    pub fn with_features(
        mut self,
        dep_kind: DependencyKind,
        mut features: Vec<&'static str>,
    ) -> Self {
        features.sort_unstable();
        self.features.push((dep_kind, features));
        self
    }

    pub fn insert_into(self, map: &mut HashMap<(PackageId, PackageId), Self>) {
        map.insert((self.from.clone(), self.to.clone()), self);
    }

    pub fn assert_metadata(&self, link: PackageLink<'_>, msg: &str) {
        let required_enabled = |status: EnabledStatus<'_>, platform: &Platform<'_>| {
            (status.required_on(platform), status.enabled_on(platform))
        };

        for (dep_kind, platform, results) in &self.platform_results {
            let req = link.req_for_kind(*dep_kind);
            assert_eq!(
                required_enabled(req.status(), platform),
                results.status,
                "{}: for platform '{}', kind {}, status is correct",
                msg,
                platform.triple(),
                dep_kind,
            );
            assert_eq!(
                required_enabled(req.default_features(), platform),
                results.default_features,
                "{}: for platform '{}', kind {}, default features is correct",
                msg,
                platform.triple(),
                dep_kind,
            );
            for (feature, status) in &results.feature_statuses {
                assert_eq!(
                    required_enabled(req.feature_status(feature), platform),
                    *status,
                    "{}: for platform '{}', kind {}, feature '{}' has correct status",
                    msg,
                    platform.triple(),
                    dep_kind,
                    feature
                );
            }
        }

        for (dep_kind, features) in &self.features {
            let metadata = link.req_for_kind(*dep_kind);
            let mut actual_features: Vec<_> = metadata.features().collect();
            actual_features.sort_unstable();
            assert_eq!(&actual_features, features, "{}: features is correct", msg);
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlatformResults {
    // Each pair stands for (required on, enabled on).
    status: (EnabledTernary, EnabledTernary),
    default_features: (EnabledTernary, EnabledTernary),
    feature_statuses: HashMap<String, (EnabledTernary, EnabledTernary)>,
}

impl PlatformResults {
    pub fn new(
        status: (EnabledTernary, EnabledTernary),
        default_features: (EnabledTernary, EnabledTernary),
    ) -> Self {
        Self {
            status,
            default_features,
            feature_statuses: HashMap::new(),
        }
    }

    pub fn with_feature_status(
        mut self,
        feature: &str,
        status: (EnabledTernary, EnabledTernary),
    ) -> Self {
        self.feature_statuses.insert(feature.to_string(), status);
        self
    }
}
