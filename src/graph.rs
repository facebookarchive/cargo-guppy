// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::errors::Error;
use auto_enums::auto_enum;
use cargo_metadata::{
    Dependency, DependencyKind, Metadata, MetadataCommand, NodeDep, Package, PackageId, Resolve,
};
use petgraph::prelude::*;
use semver::{Version, VersionReq};
use std::collections::{BTreeSet, HashMap};

#[derive(Clone, Debug)]
pub struct PackageGraph {
    // Source of truth data.
    packages: HashMap<PackageId, PackageMetadata>,
    dep_graph: Graph<PackageId, DependencyEdge>,

    // Caches, already present at construction time.

    // workspace_members is a BTreeSet so that its return value is ordered.
    workspace_members: BTreeSet<PackageId>,
}
impl PackageGraph {
    pub fn from_command(command: &mut MetadataCommand) -> Result<Self, Error> {
        Self::new(command.exec().map_err(Error::CommandError)?)
    }

    pub fn new(metadata: Metadata) -> Result<Self, Error> {
        let resolve = metadata.resolve.ok_or_else(|| {
            Error::DepGraphError(
                "no 'resolve' entries found: ensure you don't have no_deps set".into(),
            )
        })?;

        let workspace_members: BTreeSet<_> = metadata.workspace_members.into_iter().collect();

        let mut build_state = GraphBuildState::new(&metadata.packages, resolve, &workspace_members);

        let packages: HashMap<_, _> = metadata
            .packages
            .into_iter()
            .map(|package| build_state.process_package(package))
            .collect::<Result<_, _>>()?;

        let dep_graph = build_state.finish();

        Ok(Self {
            packages,
            dep_graph,
            workspace_members,
        })
    }

    /// Verifies internal invariants on this graph.
    pub fn verify(&self) -> Result<(), Error> {
        for (package_id, metadata) in self.packages() {
            for dep in self.deps(package_id) {
                let to_id = dep.to.id();
                let to_version = dep.to.version();

                let version_check = |dep_metadata: &DependencyMetadata, kind: DependencyKind| {
                    let req = dep_metadata.req();
                    if req.matches(to_version) {
                        Ok(())
                    } else {
                        Err(Error::DepGraphInternalError(format!(
                            "{} -> {} ({}): version ({}) doesn't match requirement ({})",
                            package_id,
                            to_id,
                            kind_str(kind),
                            to_version,
                            req,
                        )))
                    }
                };

                // Two invariants:
                // 1. At least one of the edges should be specified.
                // 2. The specified package should match the version dependency.
                let mut edge_set = false;
                if let Some(dep_metadata) = &dep.edge.normal {
                    edge_set = true;
                    version_check(dep_metadata, DependencyKind::Normal)?;
                }
                if let Some(dep_metadata) = &dep.edge.build {
                    edge_set = true;
                    version_check(dep_metadata, DependencyKind::Build)?;
                }
                if let Some(dep_metadata) = &dep.edge.dev {
                    edge_set = true;
                    version_check(dep_metadata, DependencyKind::Development)?;
                }

                if !edge_set {
                    return Err(Error::DepGraphInternalError(format!(
                        "{} -> {}: no edge info found",
                        package_id, to_id,
                    )));
                }
            }
        }

        Ok(())
    }

    /// Returns a list of workspace members, sorted by package ID.
    pub fn workspace_members(&self) -> impl Iterator<Item = &PackageId> + ExactSizeIterator {
        self.workspace_members.iter()
    }

    pub fn in_workspace(&self, package_id: &PackageId) -> bool {
        self.workspace_members.contains(package_id)
    }

    pub fn package_ids(&self) -> impl Iterator<Item = &PackageId> {
        self.packages.keys()
    }

    pub fn packages(&self) -> impl Iterator<Item = (&PackageId, &PackageMetadata)> {
        self.packages.iter()
    }

    pub fn metadata(&self, package_id: &PackageId) -> Option<&PackageMetadata> {
        self.packages.get(package_id)
    }

    pub fn deps<'a>(&'a self, package_id: &PackageId) -> impl Iterator<Item = PackageDep<'a>> + 'a {
        self.deps_directed(package_id, Outgoing)
    }

    pub fn reverse_deps<'a>(
        &'a self,
        package_id: &PackageId,
    ) -> impl Iterator<Item = PackageDep<'a>> + 'a {
        self.deps_directed(package_id, Incoming)
    }

    #[auto_enum]
    fn deps_directed<'a>(
        &'a self,
        package_id: &PackageId,
        dir: Direction,
    ) -> impl Iterator<Item = PackageDep<'a>> + 'a {
        #[auto_enum(Iterator)]
        match self.metadata(package_id) {
            Some(metadata) => {
                self.dep_graph
                    .edges_directed(metadata.node_idx, dir)
                    .map(move |edge| {
                        let from = self
                            .metadata(&self.dep_graph[edge.source()])
                            .expect("'from' should have associated metadata");
                        let to = self
                            .metadata(&self.dep_graph[edge.target()])
                            .expect("'to' should have associated metadata");
                        let edge = edge.weight();
                        PackageDep { from, to, edge }
                    })
            }
            None => ::std::iter::empty(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PackageDep<'a> {
    pub from: &'a PackageMetadata,
    pub to: &'a PackageMetadata,
    pub edge: &'a DependencyEdge,
}

#[derive(Clone, Debug)]
pub struct PackageMetadata {
    // Fields extracted from the package.
    id: PackageId,
    name: String,
    version: Version,
    authors: Vec<String>,
    description: Option<String>,
    license: Option<String>,
    deps: Vec<Dependency>,

    // Other information.
    node_idx: NodeIndex<u32>,
    in_workspace: bool,
    resolved_deps: Vec<NodeDep>,
    resolved_features: Vec<String>,
}

impl PackageMetadata {
    pub fn id(&self) -> &PackageId {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &Version {
        &self.version
    }

    pub fn authors(&self) -> &[String] {
        &self.authors
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_ref().map(|x| x.as_str())
    }

    pub fn license(&self) -> Option<&str> {
        self.license.as_ref().map(|x| x.as_str())
    }
}

#[derive(Clone, Debug)]
pub struct DependencyEdge {
    dep_name: String,
    resolved_name: String,
    normal: Option<DependencyMetadata>,
    build: Option<DependencyMetadata>,
    dev: Option<DependencyMetadata>,
}

impl DependencyEdge {
    /// Returns the name for this dependency edge. This can be affected by a crate rename.
    pub fn dep_name(&self) -> &str {
        &self.dep_name
    }

    /// Returns the resolved name for this dependency edge. This may involve renaming the crate and
    /// replacing - with _.
    pub fn resolved_name(&self) -> &str {
        &self.resolved_name
    }

    pub fn normal(&self) -> Option<&DependencyMetadata> {
        self.normal.as_ref()
    }

    pub fn build(&self) -> Option<&DependencyMetadata> {
        self.build.as_ref()
    }

    pub fn dev(&self) -> Option<&DependencyMetadata> {
        // XXX should dev dependencies fall back to normal if no dev-specific data was found?
        self.dev.as_ref()
    }
}

#[derive(Clone, Debug)]
pub struct DependencyMetadata {
    // Normal/dev/build can have different version requirements even if they resolve to the same
    // version.
    req: VersionReq,
    optional: bool,
    uses_default_features: bool,
    features: Vec<String>,
    target: Option<String>,
}

impl DependencyMetadata {
    pub fn req(&self) -> &VersionReq {
        &self.req
    }

    pub fn optional(&self) -> bool {
        self.optional
    }

    pub fn uses_default_features(&self) -> bool {
        self.uses_default_features
    }

    pub fn features(&self) -> &[String] {
        &self.features
    }

    pub fn target(&self) -> Option<&str> {
        self.target.as_ref().map(|x| x.as_str())
    }
}

/// Helper struct for building up dependency graph.
struct GraphBuildState<'a> {
    dep_graph: Graph<PackageId, DependencyEdge>,
    // The values of package_data are (node_idx, name, version).
    package_data: HashMap<PackageId, (NodeIndex<u32>, String, Version)>,
    resolve_data: HashMap<PackageId, (Vec<NodeDep>, Vec<String>)>,
    workspace_members: &'a BTreeSet<PackageId>,
}

impl<'a> GraphBuildState<'a> {
    fn new<'b>(
        packages: impl IntoIterator<Item = &'b Package>,
        resolve: Resolve,
        workspace_members: &'a BTreeSet<PackageId>,
    ) -> Self {
        let mut dep_graph = Graph::new();
        let package_data: HashMap<_, _> = packages
            .into_iter()
            .map(|package| {
                let node_idx = dep_graph.add_node(package.id.clone());
                (
                    package.id.clone(),
                    (node_idx, package.name.clone(), package.version.clone()),
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
            workspace_members,
        }
    }

    fn process_package(&mut self, package: Package) -> Result<(PackageId, PackageMetadata), Error> {
        let (node_idx, _, _) = self.package_data(&package.id)?;
        let in_workspace = self.workspace_members.contains(&package.id);
        let (resolved_deps, resolved_features) =
            self.resolve_data.remove(&package.id).ok_or_else(|| {
                Error::DepGraphError(format!(
                    "no resolved dependency data found for package '{}'",
                    package.id
                ))
            })?;

        // Build an index from resolved name to dependency name and known dependency data from
        // package.dependencies.
        let mut resolved_name_to_dep: HashMap<String, (&str, Vec<_>)> = HashMap::new();
        for dep in &package.dependencies {
            let dep_name = dep.rename.as_ref().unwrap_or(&dep.name);
            // cargo automatically turns - to _ during resolution.
            let resolved_name = dep_name.replace("-", "_");
            let (_, deps) = resolved_name_to_dep
                .entry(resolved_name)
                .or_insert_with(|| (dep_name, vec![]));
            deps.push(dep);
        }

        for NodeDep {
            name: resolved_name,
            pkg,
            ..
        } in &resolved_deps
        {
            let (name, deps) = resolved_name_to_dep.get(resolved_name).ok_or_else(|| {
                Error::DepGraphError(format!(
                    "{}: no dependencies found for '{}'",
                    package.id, resolved_name
                ))
            })?;
            // TODO: handle renamed packages -- the current handling is incorrect
            let (dep_idx, _, _) = self.package_data(pkg)?;
            let edge = DependencyEdge::new(name, resolved_name, deps)?;
            self.dep_graph.add_edge(node_idx, dep_idx, edge);
        }

        Ok((
            package.id.clone(),
            PackageMetadata {
                id: package.id,
                name: package.name,
                version: package.version,
                authors: package.authors,
                description: package.description,
                license: package.license,
                deps: package.dependencies,

                node_idx,
                in_workspace,
                resolved_deps,
                resolved_features,
            },
        ))
    }

    fn package_data(&self, id: &PackageId) -> Result<(NodeIndex<u32>, &str, &Version), Error> {
        let (node_idx, name, version) = self.package_data.get(&id).ok_or_else(|| {
            Error::DepGraphError(format!("no package data found for package '{}'", id))
        })?;
        Ok((*node_idx, name, version))
    }

    fn finish(self) -> Graph<PackageId, DependencyEdge> {
        self.dep_graph
    }
}

impl DependencyEdge {
    fn new(name: &str, resolved_name: &str, deps: &Vec<&Dependency>) -> Result<Self, Error> {
        // deps should have at most 1 normal dependency, 1 build dep and 1 dev dep.
        let mut normal = None;
        let mut build = None;
        let mut dev = None;
        for &dep in deps {
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
                req: dep.req.clone(),
                optional: dep.optional,
                uses_default_features: dep.uses_default_features,
                features: dep.features.clone(),
                target: dep.target.as_ref().map(|t| format!("{}", t)),
            };
            if let Some(old) = to_set.replace(metadata) {
                return Err(Error::DepGraphError(format!(
                    "Duplicate dependencies found for '{}' (kind: {})",
                    name,
                    kind_str(dep.kind)
                )));
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

fn kind_str(kind: DependencyKind) -> &'static str {
    match kind {
        DependencyKind::Normal => "normal",
        DependencyKind::Build => "build",
        DependencyKind::Development => "dev",
        _ => "unknown",
    }
}
