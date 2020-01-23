// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::{
    kind_str, DependencyEdge, DependencyMetadata, PackageGraph, PackageGraphData, PackageIx,
    PackageMetadata, Workspace,
};
use crate::{Error, Metadata, PackageId};
use cargo_metadata::{Dependency, DependencyKind, NodeDep, Package, Resolve};
use once_cell::sync::OnceCell;
use petgraph::prelude::*;
use semver::Version;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

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

struct DependencyResolver<'a> {
    from_id: &'a PackageId,

    /// The package data, inherited from the graph build state.
    package_data: &'a HashMap<PackageId, (NodeIndex<PackageIx>, String, Version)>,

    /// This is a mapping of renamed dependencies to their rename sources and dependency info --
    /// this always takes top priority.
    ///
    /// This is an owned string because hyphens can be replaced with underscores in the resolved\
    /// name. In principle this could be a Cow<'a, str>, but str::replace returns a String.
    renamed_map: HashMap<Box<str>, (&'a str, Vec<&'a Dependency>)>,

    /// This is a mapping of dependencies using their original names. For these names, dashes are
    /// not replaced with underscores.
    ///
    /// TODO: Change value type to Vec<&'a Dependency> once get_key_value is stable.
    original_map: HashMap<&'a str, (&'a str, Vec<&'a Dependency>)>,
}

impl<'a> DependencyResolver<'a> {
    /// Constructs a new resolver using the provided package data and dependencies.
    fn new(
        from_id: &'a PackageId,
        package_data: &'a HashMap<PackageId, (NodeIndex<PackageIx>, String, Version)>,
        package_deps: impl IntoIterator<Item = &'a Dependency>,
    ) -> Self {
        let mut renamed_map = HashMap::new();
        let mut original_map = HashMap::new();

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
                        .or_insert_with(|| (rename.as_str(), vec![]));
                    deps.push(dep);
                }
                Some(_) | None => {
                    let (_, deps) = original_map
                        .entry(dep.name.as_str())
                        .or_insert_with(|| (dep.name.as_str(), vec![]));
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
    fn resolve(
        &self,
        resolved_name: &str,
        package_id: &PackageId,
    ) -> Result<(&'a str, &[&'a Dependency]), Error> {
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

        // Lookup the name in the renamed map. If a hit is found here we're done.
        if let Some((name, deps)) = self.renamed_map.get(resolved_name) {
            return Ok((*name, deps));
        }

        // Lookup the package ID in the package data.
        let (_, package_name, _) = self.package_data.get(package_id).ok_or_else(|| {
            Error::PackageGraphConstructError(format!(
                "{}: no package data found for dependency '{}'",
                self.from_id, package_id
            ))
        })?;

        // Lookup the name in the original map.
        let (name, deps) = self
            .original_map
            .get(package_name.as_str())
            .ok_or_else(|| {
                Error::PackageGraphConstructError(format!(
                    "{}: no dependency information found for '{}', package ID '{}'",
                    self.from_id, package_name, package_id
                ))
            })?;
        Ok((*name, deps))
    }
}

impl DependencyEdge {
    fn new(
        from_id: &PackageId,
        name: &str,
        resolved_name: &str,
        deps: &[&Dependency],
    ) -> Result<Self, Error> {
        // deps should have at most 1 normal dependency, 1 build dep and 1 dev dep.
        let mut normal: Option<DependencyMetadata> = None;
        let mut build: Option<DependencyMetadata> = None;
        let mut dev: Option<DependencyMetadata> = None;
        for &dep in deps {
            // Dev dependencies cannot be optional.
            if dep.kind == DependencyKind::Development && dep.optional {
                return Err(Error::PackageGraphConstructError(format!(
                    "for package '{}': dev-dependency '{}' marked optional",
                    from_id, name,
                )));
            }

            let to_set = match dep.kind {
                DependencyKind::Normal => &mut normal,
                DependencyKind::Build => &mut build,
                DependencyKind::Development => &mut dev,
                _ => {
                    // unknown dependency kind -- can't do much with this!
                    continue;
                }
            };
            let metadata = DependencyMetadata {
                version_req: dep.req.clone(),
                optional: dep.optional,
                uses_default_features: dep.uses_default_features,
                features: dep.features.clone(),
                target: dep.target.as_ref().map(|t| format!("{}", t)),
            };

            // It is typically an error for the same dependency to be listed multiple times for
            // the same kind, but there are some situations in which it's possible. The main one
            // is if there's a custom 'target' field -- one real world example is at
            // https://github.com/alexcrichton/flate2-rs/blob/5751ad9/Cargo.toml#L29-L33:
            //
            // [dependencies]
            // miniz_oxide = { version = "0.3.2", optional = true}
            //
            // [target.'cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))'.dependencies]
            // miniz_oxide = "0.3.2"
            //
            // For now, prefer target = null (the more general target) in such cases, and error out
            // if both sides are null.
            //
            // TODO: Handle this better, probably through some sort of target resolution.
            let write_to_set = match to_set {
                Some(old) => match (old.target(), metadata.target()) {
                    (Some(_), None) => true,
                    (None, Some(_)) => false,
                    (Some(_), Some(_)) => {
                        // Both targets are set. We don't yet know if they are mutually exclusive,
                        // so take the first one.
                        // XXX This is wrong and needs to be fixed along with target resolution
                        // in general.
                        false
                    }
                    (None, None) => {
                        return Err(Error::PackageGraphConstructError(format!(
                            "{}: duplicate dependencies found for '{}' (kind: {})",
                            from_id,
                            name,
                            kind_str(dep.kind)
                        )))
                    }
                },
                None => true,
            };
            if write_to_set {
                to_set.replace(metadata);
            }
        }

        Ok(DependencyEdge {
            dep_name: name.into(),
            resolved_name: resolved_name.into(),
            normal,
            build,
            dev,
        })
    }
}
