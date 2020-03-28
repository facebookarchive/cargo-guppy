// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::{
    cargo_version_matches, DependencyEdge, DependencyMetadata, DependencyReq, DependencyReqImpl,
    PackageGraph, PackageGraphData, PackageIx, PackageMetadata, TargetPredicate, Workspace,
};
use crate::{Error, Metadata, PackageId, Platform};
use cargo_metadata::{Dependency, DependencyKind, NodeDep, Package, Resolve};
use once_cell::sync::OnceCell;
use petgraph::prelude::*;
use semver::{Version, VersionReq};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::mem;
use std::path::{Path, PathBuf};
use target_spec::TargetSpec;

impl PackageGraph {
    /// Constructs a new `PackageGraph` instances from the given metadata.
    pub(crate) fn build(metadata: Metadata) -> Result<Self, Error> {
        let resolve = metadata.resolve.ok_or_else(|| {
            Error::PackageGraphConstructError(
                "no 'resolve' entries found: ensure you don't have no_deps set".into(),
            )
        })?;

        let workspace_members: HashSet<_> = metadata.workspace_members.into_iter().collect();

        let mut build_state = GraphBuildState::new(
            &metadata.packages,
            resolve,
            &metadata.workspace_root,
            &workspace_members,
        );

        let packages: HashMap<_, _> = metadata
            .packages
            .into_iter()
            .map(|package| build_state.process_package(package))
            .collect::<Result<_, _>>()?;

        let dep_graph = build_state.finish();

        let workspace = Workspace::new(metadata.workspace_root, &packages, workspace_members)?;

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

impl Workspace {
    /// Indexes and creates a new workspace.
    fn new(
        workspace_root: impl Into<PathBuf>,
        packages: &HashMap<PackageId, PackageMetadata>,
        members: impl IntoIterator<Item = PackageId>,
    ) -> Result<Self, Error> {
        let workspace_root = workspace_root.into();
        // Build up the workspace members by path, since most interesting queries are going to
        // happen by path.
        let members_by_path = members
            .into_iter()
            .map(|id| {
                // Strip off the workspace path from the manifest path.
                let package_metadata = packages.get(&id).ok_or_else(|| {
                    Error::PackageGraphConstructError(format!(
                        "workspace member '{}' not found",
                        id
                    ))
                })?;
                let workspace_path = package_metadata.workspace_path().ok_or_else(|| {
                    Error::PackageGraphConstructError(format!(
                        "workspace member '{}' at path {:?} not in workspace",
                        id,
                        package_metadata.manifest_path(),
                    ))
                })?;
                Ok((workspace_path.to_path_buf(), id))
            })
            .collect::<Result<BTreeMap<PathBuf, PackageId>, Error>>()?;

        Ok(Self {
            root: workspace_root,
            members_by_path,
        })
    }
}

/// Helper struct for building up dependency graph.
struct GraphBuildState<'a> {
    dep_graph: Graph<PackageId, DependencyEdge, Directed, PackageIx>,
    // The values of package_data are (package_ix, name, version).
    package_data: HashMap<PackageId, (NodeIndex<PackageIx>, String, Version)>,
    resolve_data: HashMap<PackageId, (Vec<NodeDep>, Vec<String>)>,
    workspace_root: &'a Path,
    workspace_members: &'a HashSet<PackageId>,
}

impl<'a> GraphBuildState<'a> {
    fn new(
        packages: &[Package],
        resolve: Resolve,
        workspace_root: &'a Path,
        workspace_members: &'a HashSet<PackageId>,
    ) -> Self {
        // No idea how many edges there are going to be, so use packages.len() as a reasonable lower
        // bound.
        let mut dep_graph = Graph::with_capacity(packages.len(), packages.len());
        let package_data: HashMap<_, _> = packages
            .iter()
            .map(|package| {
                let package_ix = dep_graph.add_node(package.id.clone());
                (
                    package.id.clone(),
                    (package_ix, package.name.clone(), package.version.clone()),
                )
            })
            .collect();

        let resolve_data: HashMap<_, _> = resolve
            .nodes
            .into_iter()
            .map(|node| (node.id, (node.deps, node.features)))
            .collect();

        Self {
            dep_graph,
            package_data,
            resolve_data,
            workspace_root,
            workspace_members,
        }
    }

    fn process_package(&mut self, package: Package) -> Result<(PackageId, PackageMetadata), Error> {
        let (package_ix, _, _) = self.package_data(&package.id)?;

        let workspace_path = if self.workspace_members.contains(&package.id) {
            Some(self.workspace_path(&package.id, &package.manifest_path)?)
        } else {
            None
        };

        let (resolved_deps, resolved_features) =
            self.resolve_data.remove(&package.id).ok_or_else(|| {
                Error::PackageGraphConstructError(format!(
                    "no resolved dependency data found for package '{}'",
                    package.id
                ))
            })?;

        let dep_resolver =
            DependencyResolver::new(&package.id, &self.package_data, &package.dependencies);

        for NodeDep {
            name: resolved_name,
            pkg,
            ..
        } in &resolved_deps
        {
            let (name, deps) = dep_resolver.resolve(resolved_name, pkg)?;
            let (dep_idx, _, _) = self.package_data(pkg)?;
            let edge = DependencyEdge::new(&package.id, name, resolved_name, deps)?;
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

        Ok((
            package.id.clone(),
            PackageMetadata {
                id: package.id,
                name: package.name,
                version: package.version,
                authors: package.authors,
                description: package.description.map(|s| s.into()),
                license: package.license.map(|s| s.into()),
                license_file: package.license_file.map(|s| s.into()),
                manifest_path: package.manifest_path.into(),
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
                workspace_path,
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
    fn workspace_path(&self, id: &PackageId, manifest_path: &Path) -> Result<Box<Path>, Error> {
        // Strip off the workspace path from the manifest path.
        let workspace_path = manifest_path
            .strip_prefix(self.workspace_root)
            .map_err(|_| {
                Error::PackageGraphConstructError(format!(
                    "workspace member '{}' at path {:?} not in workspace (root: {:?})",
                    id, manifest_path, self.workspace_root
                ))
            })?;
        let workspace_path = workspace_path.parent().ok_or_else(|| {
            Error::PackageGraphConstructError(format!(
                "workspace member '{}' has invalid manifest path {:?}",
                id, manifest_path
            ))
        })?;
        Ok(workspace_path.to_path_buf().into_boxed_path())
    }

    fn finish(self) -> Graph<PackageId, DependencyEdge, Directed, PackageIx> {
        self.dep_graph
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

impl DependencyEdge {
    fn new<'a>(
        from_id: &PackageId,
        name: &str,
        resolved_name: &str,
        deps: impl IntoIterator<Item = &'a Dependency>,
    ) -> Result<Self, Error> {
        let mut normal = DependencyBuildState::default();
        let mut build = DependencyBuildState::default();
        let mut dev = DependencyBuildState::default();
        for dep in deps {
            // Dev dependencies cannot be optional.
            if dep.kind == DependencyKind::Development && dep.optional {
                return Err(Error::PackageGraphConstructError(format!(
                    "for package '{}': dev-dependency '{}' marked optional",
                    from_id, name,
                )));
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

        Ok(DependencyEdge {
            dep_name: name.into(),
            resolved_name: resolved_name.into(),
            normal: normal.finish()?,
            build: build.finish()?,
            dev: dev.finish()?,
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
///    separately for mandatory/optional instances.
///
/// Note that the new feature resolver
/// (https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#features)'s `itarget` setting
/// causes this union-ing to *not* happen, so that's why we store all the features enabled by
/// each target separately.
#[derive(Debug, Default)]
struct DependencyBuildState {
    // This is the `req` field from the first instance seen if there are any, or `None` if none are
    // seen.
    version_req: Option<VersionReq>,
    dependency_req: DependencyReq,
    // Set if there's a single target -- mostly there for backwards compat support.
    single_target: Option<String>,
}

impl DependencyBuildState {
    fn add_instance(&mut self, from_id: &PackageId, dep: &Dependency) -> Result<(), Error> {
        match &self.version_req {
            Some(_) => {
                // There's more than one instance, so mark the single target `None`.
                self.single_target = None;
            }
            None => {
                self.version_req = Some(dep.req.clone());
                self.single_target = dep.target.as_ref().map(|platform| format!("{}", platform));
            }
        }
        self.dependency_req.add_instance(from_id, dep)?;

        Ok(())
    }

    fn finish(self) -> Result<Option<DependencyMetadata>, Error> {
        let version_req = match self.version_req {
            Some(version_req) => version_req,
            None => {
                // No instances seen.
                return Ok(None);
            }
        };

        let dependency_req = self.dependency_req;

        // Evaluate this dependency against the current platform.
        let current_platform = Platform::current().ok_or(Error::UnknownCurrentPlatform)?;
        let current_enabled = dependency_req.enabled_on(&current_platform);
        let current_default_features = dependency_req.default_features_on(&current_platform);

        // Collect all features from both the optional and mandatory instances.
        let all_features: HashSet<_> = dependency_req.all_features().collect();
        let all_features: Vec<_> = all_features
            .into_iter()
            .map(|feature| feature.to_string())
            .collect();

        // Collect the status of every feature on this platform.
        let current_feature_statuses = all_features
            .iter()
            .map(|feature| {
                (
                    feature.clone(),
                    dependency_req.feature_enabled_on(feature, &current_platform),
                )
            })
            .collect();

        Ok(Some(DependencyMetadata {
            version_req,
            dependency_req,
            current_enabled,
            current_default_features,
            all_features,
            current_feature_statuses,
            single_target: self.single_target,
        }))
    }
}

impl DependencyReq {
    fn add_instance(&mut self, from_id: &PackageId, dep: &Dependency) -> Result<(), Error> {
        if dep.optional {
            self.optional.add_instance(from_id, dep)
        } else {
            self.mandatory.add_instance(from_id, dep)
        }
    }

    fn all_features(&self) -> impl Iterator<Item = &str> {
        self.mandatory
            .all_features()
            .chain(self.optional.all_features())
    }
}

impl DependencyReqImpl {
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
        }
        self.target_features
            .push((target_spec, dep.features.clone()));
        Ok(())
    }
}

impl TargetPredicate {
    pub(super) fn extend(&mut self, other: &TargetPredicate) {
        // &mut *self is a reborrow to allow mem::replace to work below.
        match (&mut *self, other) {
            (TargetPredicate::Always, _) => {
                // Always stays the same since it means all specs are included.
            }
            (TargetPredicate::Specs(_), TargetPredicate::Always) => {
                // Mark self as Always.
                mem::replace(self, TargetPredicate::Always);
            }
            (TargetPredicate::Specs(specs), TargetPredicate::Specs(other)) => {
                specs.extend_from_slice(other.as_slice());
            }
        }
    }

    pub(super) fn add_spec(&mut self, spec: Option<&TargetSpec>) {
        // &mut *self is a reborrow to allow mem::replace to work below.
        match (&mut *self, spec) {
            (TargetPredicate::Always, _) => {
                // Always stays the same since it means all specs are included.
            }
            (TargetPredicate::Specs(_), None) => {
                // Mark self as Always.
                mem::replace(self, TargetPredicate::Always);
            }
            (TargetPredicate::Specs(specs), Some(spec)) => {
                specs.push(spec.clone());
            }
        }
    }
}

impl Default for TargetPredicate {
    fn default() -> Self {
        // Empty vector means never.
        TargetPredicate::Specs(vec![])
    }
}
