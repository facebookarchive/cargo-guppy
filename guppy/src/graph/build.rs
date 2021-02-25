// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{
    graph::{
        cargo_version_matches, BuildTargetImpl, BuildTargetKindImpl, DepRequiredOrOptional,
        DependencyReqImpl, OwnedBuildTargetId, PackageGraph, PackageGraphData, PackageIx,
        PackageLinkImpl, PackageMetadataImpl, PackageSourceImpl, PlatformStatusImpl, WorkspaceImpl,
    },
    sorted_set::SortedSet,
    Error, PackageId,
};
use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::{Dependency, DependencyKind, Metadata, NodeDep, Package, Resolve, Target};
use once_cell::sync::OnceCell;
use petgraph::prelude::*;
use semver::Version;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
};
use target_spec::TargetSpec;

impl PackageGraph {
    /// Constructs a new `PackageGraph` instances from the given metadata.
    pub(crate) fn build(metadata: Metadata) -> Result<Self, Error> {
        let resolve = metadata.resolve.ok_or_else(|| {
            Error::PackageGraphConstructError(
                "no 'resolve' entries found: ensure you don't have no_deps set".into(),
            )
        })?;

        let workspace_members: HashSet<_> = metadata
            .workspace_members
            .into_iter()
            .map(PackageId::from_metadata)
            .collect();

        let workspace_root =
            Utf8PathBuf::from_path_buf(metadata.workspace_root).map_err(|path_buf| {
                Error::PackageGraphConstructError(format!(
                    "workspace root is invalid UTF-8: {}",
                    path_buf.display()
                ))
            })?;

        let mut build_state = GraphBuildState::new(
            &metadata.packages,
            resolve,
            &workspace_root,
            &workspace_members,
        );

        let packages: HashMap<_, _> = metadata
            .packages
            .into_iter()
            .map(|package| build_state.process_package(package))
            .collect::<Result<_, _>>()?;

        let dep_graph = build_state.finish();

        let workspace = WorkspaceImpl::new(workspace_root, &packages, workspace_members)?;

        Ok(Self {
            dep_graph,
            sccs: OnceCell::new(),
            feature_graph: OnceCell::new(),
            data: PackageGraphData {
                packages,
                workspace,
            },
        })
    }
}

impl WorkspaceImpl {
    /// Indexes and creates a new workspace.
    fn new(
        workspace_root: impl Into<Utf8PathBuf>,
        packages: &HashMap<PackageId, PackageMetadataImpl>,
        members: impl IntoIterator<Item = PackageId>,
    ) -> Result<Self, Error> {
        use std::collections::btree_map::Entry;

        let workspace_root = workspace_root.into();
        // Build up the workspace members by path, since most interesting queries are going to
        // happen by path.
        let mut members_by_path = BTreeMap::new();
        let mut members_by_name = BTreeMap::new();
        for id in members {
            // Strip off the workspace path from the manifest path.
            let package_metadata = packages.get(&id).ok_or_else(|| {
                Error::PackageGraphConstructError(format!("workspace member '{}' not found", id))
            })?;

            let workspace_path = match &package_metadata.source {
                PackageSourceImpl::Workspace(path) => path,
                _ => {
                    return Err(Error::PackageGraphConstructError(format!(
                        "workspace member '{}' at path {:?} not in workspace",
                        id, package_metadata.manifest_path,
                    )));
                }
            };
            members_by_path.insert(workspace_path.to_path_buf(), id.clone());

            match members_by_name.entry(package_metadata.name.clone().into_boxed_str()) {
                Entry::Vacant(vacant) => {
                    vacant.insert(id.clone());
                }
                Entry::Occupied(occupied) => {
                    return Err(Error::PackageGraphConstructError(format!(
                        "duplicate package name in workspace: '{}' is name for '{}' and '{}'",
                        occupied.key(),
                        occupied.get(),
                        id
                    )))
                }
            }
        }

        Ok(Self {
            root: workspace_root,
            members_by_path,
            members_by_name,
            #[cfg(feature = "proptest1")]
            name_list: OnceCell::new(),
        })
    }
}

/// Helper struct for building up dependency graph.
struct GraphBuildState<'a> {
    dep_graph: Graph<PackageId, PackageLinkImpl, Directed, PackageIx>,
    // The values of package_data are (package_ix, name, version).
    package_data: HashMap<PackageId, (NodeIndex<PackageIx>, String, Version)>,
    resolve_data: HashMap<PackageId, (Vec<NodeDep>, Vec<String>)>,
    workspace_root: &'a Utf8Path,
    workspace_members: &'a HashSet<PackageId>,
}

impl<'a> GraphBuildState<'a> {
    fn new(
        packages: &[Package],
        resolve: Resolve,
        workspace_root: &'a Utf8Path,
        workspace_members: &'a HashSet<PackageId>,
    ) -> Self {
        // No idea how many edges there are going to be, so use packages.len() as a reasonable lower
        // bound.
        let mut dep_graph = Graph::with_capacity(packages.len(), packages.len());
        let package_data: HashMap<_, _> = packages
            .iter()
            .map(|package| {
                let package_id = PackageId::from_metadata(package.id.clone());
                let package_ix = dep_graph.add_node(package_id.clone());
                (
                    package_id,
                    (package_ix, package.name.clone(), package.version.clone()),
                )
            })
            .collect();

        let resolve_data: HashMap<_, _> = resolve
            .nodes
            .into_iter()
            .map(|node| {
                (
                    PackageId::from_metadata(node.id),
                    (node.deps, node.features),
                )
            })
            .collect();

        Self {
            dep_graph,
            package_data,
            resolve_data,
            workspace_root,
            workspace_members,
        }
    }

    fn process_package(
        &mut self,
        package: Package,
    ) -> Result<(PackageId, PackageMetadataImpl), Error> {
        let package_id = PackageId::from_metadata(package.id);
        let (package_ix, _, _) = self.package_data(&package_id)?;

        let source = if self.workspace_members.contains(&package_id) {
            PackageSourceImpl::Workspace(self.workspace_path(&package_id, &package.manifest_path)?)
        } else if let Some(source) = package.source {
            if source.is_crates_io() {
                PackageSourceImpl::CratesIo
            } else {
                PackageSourceImpl::External(source.repr.into())
            }
        } else {
            // Path dependency: get the directory from the manifest path.
            let dirname = match package.manifest_path.parent() {
                Some(dirname) => dirname,
                None => {
                    return Err(Error::PackageGraphConstructError(format!(
                        "package '{}': manifest path '{}' does not have parent",
                        package_id,
                        package.manifest_path.display(),
                    )));
                }
            };
            let rel_path = pathdiff::diff_paths(dirname, self.workspace_root)
                .expect("workspace root is absolute");
            let rel_path = Utf8PathBuf::from_path_buf(rel_path).map_err(|path_buf| {
                Error::PackageGraphConstructError(format!(
                    "package '{}': location '{}' is invalid UTF-8",
                    package_id,
                    path_buf.display()
                ))
            })?;
            PackageSourceImpl::Path(rel_path.into_boxed_path())
        };

        let mut build_targets = BuildTargets::new(&package_id);
        for build_target in package.targets {
            build_targets.add(build_target)?;
        }
        let build_targets = build_targets.finish();

        let (resolved_deps, resolved_features) =
            self.resolve_data.remove(&package_id).ok_or_else(|| {
                Error::PackageGraphConstructError(format!(
                    "no resolved dependency data found for package '{}'",
                    package_id
                ))
            })?;

        let dep_resolver =
            DependencyResolver::new(&package_id, &self.package_data, &package.dependencies);

        for NodeDep {
            name: resolved_name,
            pkg,
            ..
        } in &resolved_deps
        {
            let dep_id = PackageId::from_metadata(pkg.clone());
            let (name, deps) = dep_resolver.resolve(resolved_name, &dep_id)?;
            let (dep_idx, _, _) = self.package_data(&dep_id)?;
            let edge = PackageLinkImpl::new(&package_id, name, resolved_name, deps)?;
            // Use update_edge instead of add_edge to prevent multiple edges from being added
            // between these two nodes.
            // XXX maybe check for an existing edge?
            self.dep_graph.update_edge(package_ix, dep_idx, edge);
        }

        let has_default_feature = package.features.contains_key("default");

        // Optional dependencies could in principle be computed by looking at the edges out of this
        // package, but unresolved dependencies aren't part of the graph so we're going to miss them
        // (and many optional dependencies will be unresolved).
        //
        // XXX: This might be something to revisit if we start modeling unresolved dependencies in
        // the graph.
        //
        // A dependency might be listed multiple times (e.g. as a build dependency and as a normal
        // one). Some of them might be optional, some might not be. List a dependency here if *any*
        // of those specifications are optional, since that's how Cargo features work. But also
        // dedup them.
        let optional_deps = package
            .dependencies
            .into_iter()
            .filter_map(|dep| {
                if dep.optional {
                    match dep.rename {
                        Some(rename) => Some(rename.into_boxed_str()),
                        None => Some(dep.name.into_boxed_str()),
                    }
                } else {
                    None
                }
            })
            .map(|feature| (feature, None));

        // The feature map contains both optional deps and named features.
        let features = package
            .features
            .into_iter()
            .map(|(feature, deps)| (feature.into_boxed_str(), Some(deps)))
            .chain(optional_deps)
            .collect();

        let license_file = match package.license_file {
            Some(license_file) => Some(
                Utf8PathBuf::from_path_buf(license_file)
                    .map_err(|path_buf| {
                        Error::PackageGraphConstructError(format!(
                            "for package '{}', license file is invalid UTF-8: {}",
                            package_id,
                            path_buf.display()
                        ))
                    })?
                    .into(),
            ),
            None => None,
        };

        let manifest_path = Utf8PathBuf::from_path_buf(package.manifest_path)
            .map_err(|path_buf| {
                Error::PackageGraphConstructError(format!(
                    "for package '{}', manifest path is invalid UTF-8: {}",
                    package_id,
                    path_buf.display()
                ))
            })?
            .into();

        Ok((
            package_id,
            PackageMetadataImpl {
                name: package.name,
                version: package.version,
                authors: package.authors,
                description: package.description.map(|s| s.into()),
                license: package.license.map(|s| s.into()),
                license_file,
                manifest_path,
                categories: package.categories,
                keywords: package.keywords,
                readme: package.readme.map(|s| s.into()),
                repository: package.repository.map(|s| s.into()),
                edition: package.edition.into(),
                metadata_table: package.metadata,
                links: package.links.map(|s| s.into()),
                publish: package.publish,
                features,

                package_ix,
                source,
                build_targets,
                has_default_feature,
                resolved_deps,
                resolved_features,
            },
        ))
    }

    fn package_data(
        &self,
        id: &PackageId,
    ) -> Result<(NodeIndex<PackageIx>, &str, &Version), Error> {
        let (package_ix, name, version) = self.package_data.get(&id).ok_or_else(|| {
            Error::PackageGraphConstructError(format!("no package data found for package '{}'", id))
        })?;
        Ok((*package_ix, name, version))
    }

    /// Computes the workspace path for this package. Errors if this package is not in the
    /// workspace.
    fn workspace_path(&self, id: &PackageId, manifest_path: &Path) -> Result<Box<Utf8Path>, Error> {
        // Strip off the workspace path from the manifest path.
        let workspace_path = manifest_path
            .strip_prefix(self.workspace_root)
            .map_err(|_| {
                Error::PackageGraphConstructError(format!(
                    "workspace member '{}' at path {:?} not in workspace (root: {})",
                    id, manifest_path, self.workspace_root
                ))
            })?;
        let workspace_path = workspace_path.parent().ok_or_else(|| {
            Error::PackageGraphConstructError(format!(
                "workspace member '{}' has invalid manifest path {:?}",
                id, manifest_path
            ))
        })?;
        let workspace_path = Utf8Path::from_path(workspace_path).ok_or_else(|| {
            Error::PackageGraphConstructError(format!(
                "workspace member '{}' has invalid UTF-8 manifest path {:?}",
                id, manifest_path
            ))
        })?;
        Ok(workspace_path.to_path_buf().into_boxed_path())
    }

    fn finish(self) -> Graph<PackageId, PackageLinkImpl, Directed, PackageIx> {
        self.dep_graph
    }
}

struct BuildTargets<'a> {
    package_id: &'a PackageId,
    targets: BTreeMap<OwnedBuildTargetId, BuildTargetImpl>,
}

impl<'a> BuildTargets<'a> {
    fn new(package_id: &'a PackageId) -> Self {
        Self {
            package_id,
            targets: BTreeMap::new(),
        }
    }

    fn add(&mut self, target: Target) -> Result<(), Error> {
        use std::collections::btree_map::Entry;

        // Figure out the id and kind using target.kind and target.crate_types.
        let mut target_kinds = target.kind;
        let target_name = target.name.into_boxed_str();
        let crate_types = SortedSet::new(target.crate_types);

        // The "proc-macro" crate type cannot mix with any other types or kinds.
        if target_kinds.len() > 1 && Self::is_proc_macro(&target_kinds) {
            return Err(Error::PackageGraphConstructError(format!(
                "for package {}, proc-macro mixed with other kinds ({:?})",
                self.package_id, target_kinds
            )));
        }
        if crate_types.len() > 1 && Self::is_proc_macro(&crate_types) {
            return Err(Error::PackageGraphConstructError(format!(
                "for package {}, proc-macro mixed with other crate types ({})",
                self.package_id, crate_types
            )));
        }

        let (id, kind, lib_name) = if target_kinds.len() > 1 {
            // multiple kinds always means a library target.
            (
                OwnedBuildTargetId::Library,
                BuildTargetKindImpl::LibraryOrExample(crate_types),
                Some(target_name),
            )
        } else if let Some(target_kind) = target_kinds.pop() {
            let (id, lib_name) = match target_kind.as_str() {
                "custom-build" => (OwnedBuildTargetId::BuildScript, Some(target_name)),
                "bin" => (OwnedBuildTargetId::Binary(target_name), None),
                "example" => (OwnedBuildTargetId::Example(target_name), None),
                "test" => (OwnedBuildTargetId::Test(target_name), None),
                "bench" => (OwnedBuildTargetId::Benchmark(target_name), None),
                _other => {
                    // Assume that this is a library crate.
                    (OwnedBuildTargetId::Library, Some(target_name))
                }
            };

            let kind = match &id {
                OwnedBuildTargetId::Library => {
                    if crate_types.as_slice() == ["proc-macro"] {
                        BuildTargetKindImpl::ProcMacro
                    } else {
                        BuildTargetKindImpl::LibraryOrExample(crate_types)
                    }
                }
                OwnedBuildTargetId::Example(_) => {
                    BuildTargetKindImpl::LibraryOrExample(crate_types)
                }
                _ => {
                    // The crate_types must be exactly "bin".
                    if crate_types.as_slice() != ["bin"] {
                        return Err(Error::PackageGraphConstructError(format!(
                            "for package {}: build target '{:?}' has invalid crate types '{}'",
                            self.package_id, id, crate_types,
                        )));
                    }
                    BuildTargetKindImpl::Binary
                }
            };

            (id, kind, lib_name)
        } else {
            return Err(Error::PackageGraphConstructError(format!(
                "for package ID '{}': build target '{}' has no kinds",
                self.package_id, target_name
            )));
        };

        match self.targets.entry(id) {
            Entry::Occupied(occupied) => {
                return Err(Error::PackageGraphConstructError(format!(
                    "for package ID '{}': duplicate build targets for {:?}",
                    self.package_id,
                    occupied.key()
                )));
            }
            Entry::Vacant(vacant) => {
                vacant.insert(BuildTargetImpl {
                    kind,
                    lib_name,
                    required_features: target.required_features,
                    path: target.src_path.into_boxed_path(),
                    edition: target.edition.into_boxed_str(),
                    doc_tests: target.doctest,
                });
            }
        }

        Ok(())
    }

    fn is_proc_macro(list: &[String]) -> bool {
        list.iter().any(|kind| kind.as_str() == "proc-macro")
    }

    fn finish(self) -> BTreeMap<OwnedBuildTargetId, BuildTargetImpl> {
        self.targets
    }
}

struct DependencyResolver<'g> {
    from_id: &'g PackageId,

    /// The package data, inherited from the graph build state.
    package_data: &'g HashMap<PackageId, (NodeIndex<PackageIx>, String, Version)>,

    /// This is a mapping of renamed dependencies to their rename sources and dependency info --
    /// this always takes top priority.
    ///
    /// This is an owned string because hyphens can be replaced with underscores in the resolved\
    /// name. In principle this could be a Cow<'a, str>, but str::replace returns a String.
    renamed_map: HashMap<Box<str>, (&'g str, DependencyReqs<'g>)>,

    /// This is a mapping of dependencies using their original names. For these names, dashes are
    /// not replaced with underscores.
    original_map: HashMap<&'g str, DependencyReqs<'g>>,
}

impl<'g> DependencyResolver<'g> {
    /// Constructs a new resolver using the provided package data and dependencies.
    fn new(
        from_id: &'g PackageId,
        package_data: &'g HashMap<PackageId, (NodeIndex<PackageIx>, String, Version)>,
        package_deps: impl IntoIterator<Item = &'g Dependency>,
    ) -> Self {
        let mut renamed_map = HashMap::new();
        let mut original_map: HashMap<_, DependencyReqs<'g>> = HashMap::new();

        for dep in package_deps {
            match &dep.rename {
                // The rename != dep.name check is because of Cargo.toml instances like this:
                //
                // [dependencies]
                // datatest = "0.4.2"
                //
                // [build-dependencies]
                // datatest = { package = "datatest", version = "0.4.2" }
                //
                // cargo seems to accept such cases if the name doesn't contain a hyphen.
                Some(rename) if rename != &dep.name => {
                    // The resolved name is the same as the renamed name, except dashes are replaced
                    // with underscores.
                    let resolved_name = rename.replace("-", "_");
                    let (_, deps) = renamed_map
                        .entry(resolved_name.into())
                        .or_insert_with(|| (rename.as_str(), DependencyReqs::default()));
                    deps.push(dep);
                }
                Some(_) | None => {
                    let deps = original_map.entry(dep.name.as_str()).or_default();
                    deps.push(dep);
                }
            }
        }

        Self {
            from_id,
            package_data,
            renamed_map,
            original_map,
        }
    }

    /// Resolves this dependency by finding the `Dependency` corresponding to this resolved name
    /// and package ID.
    fn resolve<'a>(
        &'a self,
        resolved_name: &str,
        package_id: &PackageId,
    ) -> Result<(&'g str, impl Iterator<Item = &'g Dependency> + 'a), Error> {
        // This method needs to reconcile three separate sources of data:
        // 1. The metadata for each package, which is basically a parsed version of the Cargo.toml
        //    for that package.
        // 2. The list of dependencies for the source package, which is also extracted from
        //    Cargo.toml for that package.
        // 3. The "resolve" section of the manifest, which has resolved names and package IDs (this
        //    is what's passed in).
        //
        // The below algorithm does a pretty job, but there are some edge cases it has trouble
        // handling, primarily around malformed Cargo.toml files. For example, this Cargo.toml file
        // will result in a metadata JSON (as of Rust 1.37) that will parse incorrectly:
        //
        // [dependencies]
        // lazy_static = "1"
        //
        // [build-dependencies]
        // lazy_static_new = { package = "lazy_static", version = "1", optional = true }
        //
        // TODO: Add detection for cases like this.

        // Lookup the package ID in the package data.
        let (_, package_name, version) = self.package_data.get(package_id).ok_or_else(|| {
            Error::PackageGraphConstructError(format!(
                "{}: no package data found for dependency '{}'",
                self.from_id, package_id
            ))
        })?;

        // ---
        // Both the following checks verify against the version as well to allow situations like
        // this to work:
        //
        // [dependencies]
        // lazy_static = "1"
        //
        // [dev-dependencies]
        // lazy_static = "0.2"
        //
        // This needs to be done against the renamed map as well.
        // ---

        // Lookup the name in the renamed map. If a hit is found here we're done.
        if let Some((name, deps)) = self.renamed_map.get(resolved_name) {
            return Ok((*name, deps.matches_for(version)));
        }

        // Lookup the name in the original map.
        let (name, dep_reqs) = self
            .original_map
            .get_key_value(package_name.as_str())
            .ok_or_else(|| {
                Error::PackageGraphConstructError(format!(
                    "{}: no dependency information found for '{}', package ID '{}'",
                    self.from_id, package_name, package_id
                ))
            })?;
        let deps = dep_reqs.matches_for(version);
        Ok((*name, deps))
    }
}

/// Maintains a list of dependency requirements to match up to for a given package name.
#[derive(Clone, Debug, Default)]
struct DependencyReqs<'g> {
    reqs: Vec<&'g Dependency>,
}

impl<'g> DependencyReqs<'g> {
    fn push(&mut self, dependency: &'g Dependency) {
        self.reqs.push(dependency);
    }

    fn matches_for<'a>(
        &'a self,
        version: &'a Version,
    ) -> impl Iterator<Item = &'g Dependency> + 'a {
        self.reqs.iter().filter_map(move |dep| {
            if cargo_version_matches(&dep.req, version) {
                Some(*dep)
            } else {
                None
            }
        })
    }
}

impl PackageLinkImpl {
    fn new<'a>(
        from_id: &PackageId,
        name: &str,
        resolved_name: &str,
        deps: impl IntoIterator<Item = &'a Dependency>,
    ) -> Result<Self, Error> {
        let mut version_req = None;
        let mut normal = DependencyReqImpl::default();
        let mut build = DependencyReqImpl::default();
        let mut dev = DependencyReqImpl::default();
        for dep in deps {
            // Dev dependencies cannot be optional.
            if dep.kind == DependencyKind::Development && dep.optional {
                return Err(Error::PackageGraphConstructError(format!(
                    "for package '{}': dev-dependency '{}' marked optional",
                    from_id, name,
                )));
            }

            // Pick the first version req that this come across.
            if version_req.is_none() {
                version_req = Some(dep.req.clone());
            }

            match dep.kind {
                DependencyKind::Normal => normal.add_instance(from_id, dep)?,
                DependencyKind::Build => build.add_instance(from_id, dep)?,
                DependencyKind::Development => dev.add_instance(from_id, dep)?,
                _ => {
                    // unknown dependency kind -- can't do much with this!
                    continue;
                }
            };
        }

        Ok(Self {
            dep_name: name.into(),
            resolved_name: resolved_name.into(),
            version_req: version_req.expect("at least one dependency instance"),
            normal,
            build,
            dev,
        })
    }
}

/// It is possible to specify a dependency several times within the same section through
/// platform-specific dependencies and the [target] section. For example:
/// https://github.com/alexcrichton/flate2-rs/blob/5751ad9/Cargo.toml#L29-L33
///
/// ```toml
/// [dependencies]
/// miniz_oxide = { version = "0.3.2", optional = true}
///
/// [target.'cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))'.dependencies]
/// miniz_oxide = "0.3.2"
/// ```
///
/// (From here on, each separate time a particular version of a dependency
/// is listed, it is called an "instance".)
///
/// For such situations, there are two separate analyses that happen:
///
/// 1. Whether the dependency is included at all. This is a union of all instances, conditional on
///    the specifics of the `[target]` lines.
/// 2. What features are enabled. As of cargo 1.42, this is unified across all instances but
///    separately for required/optional instances.
///
/// Note that the new feature resolver
/// (https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#features)'s `itarget` setting
/// causes this union-ing to *not* happen, so that's why we store all the features enabled by
/// each target separately.
impl DependencyReqImpl {
    fn add_instance(&mut self, from_id: &PackageId, dep: &Dependency) -> Result<(), Error> {
        if dep.optional {
            self.optional.add_instance(from_id, dep)
        } else {
            self.required.add_instance(from_id, dep)
        }
    }
}

impl DepRequiredOrOptional {
    fn add_instance(&mut self, from_id: &PackageId, dep: &Dependency) -> Result<(), Error> {
        // target_spec is None if this is not a platform-specific dependency.
        let target_spec = match dep.target.as_ref() {
            Some(spec_or_triple) => {
                // This is a platform-specific dependency, so add it to the list of specs.
                let spec_or_triple = format!("{}", spec_or_triple);
                let target_spec: TargetSpec = spec_or_triple.parse().map_err(|err| {
                    Error::PackageGraphConstructError(format!(
                        "for package '{}': for dependency '{}', parsing target '{}' failed: {}",
                        from_id, dep.name, spec_or_triple, err
                    ))
                })?;
                Some(target_spec)
            }
            None => None,
        };

        self.build_if.add_spec(target_spec.as_ref());
        if dep.uses_default_features {
            self.default_features_if.add_spec(target_spec.as_ref());
        } else {
            self.no_default_features_if.add_spec(target_spec.as_ref());
        }

        for feature in &dep.features {
            self.feature_targets
                .entry(feature.clone())
                .or_default()
                .add_spec(target_spec.as_ref());
        }
        Ok(())
    }
}

impl PlatformStatusImpl {
    pub(super) fn extend(&mut self, other: &PlatformStatusImpl) {
        // &mut *self is a reborrow to allow mem::replace to work below.
        match (&mut *self, other) {
            (PlatformStatusImpl::Always, _) => {
                // Always stays the same since it means all specs are included.
            }
            (PlatformStatusImpl::Specs(_), PlatformStatusImpl::Always) => {
                // Mark self as Always.
                *self = PlatformStatusImpl::Always;
            }
            (PlatformStatusImpl::Specs(specs), PlatformStatusImpl::Specs(other)) => {
                specs.extend_from_slice(other.as_slice());
            }
        }
    }

    pub(super) fn add_spec(&mut self, spec: Option<&TargetSpec<'static>>) {
        // &mut *self is a reborrow to allow mem::replace to work below.
        match (&mut *self, spec) {
            (PlatformStatusImpl::Always, _) => {
                // Always stays the same since it means all specs are included.
            }
            (PlatformStatusImpl::Specs(_), None) => {
                // Mark self as Always.
                *self = PlatformStatusImpl::Always;
            }
            (PlatformStatusImpl::Specs(specs), Some(spec)) => {
                specs.push(spec.clone());
            }
        }
    }
}
