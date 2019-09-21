use crate::errors::Error;
use crate::graph::walk::EdgeBfs;
use cargo_metadata::{Dependency, DependencyKind, MetadataCommand, NodeDep, PackageId};
use lazy_static::lazy_static;
use petgraph::prelude::*;
use petgraph::visit::{Visitable, Walker};
use semver::{Version, VersionReq};
use std::collections::{BTreeMap, HashMap};
use std::iter::FromIterator;
use std::path::{Path, PathBuf};

mod build;
mod walk;

#[derive(Clone, Debug)]
pub struct PackageGraph {
    // Source of truth data.
    packages: HashMap<PackageId, PackageMetadata>,
    dep_graph: Graph<PackageId, DependencyEdge>,
    workspace: Workspace,
}
impl PackageGraph {
    pub fn from_command(command: &mut MetadataCommand) -> Result<Self, Error> {
        Self::new(command.exec().map_err(Error::CommandError)?)
    }

    // fn new() is in graph/build.rs.

    /// Verifies internal invariants on this graph.
    pub fn verify(&self) -> Result<(), Error> {
        lazy_static! {
            static ref MAJOR_WILDCARD: VersionReq = VersionReq::parse("*").unwrap();
        }

        for (package_id, metadata) in self.packages() {
            for dep in self.deps_node_idx_directed(metadata.node_idx, Outgoing) {
                let to_id = dep.to.id();
                let to_version = dep.to.version();

                let version_check = |dep_metadata: &DependencyMetadata, kind: DependencyKind| {
                    let req = dep_metadata.req();
                    // A requirement of "*" filters out pre-release versions with the semver crate,
                    // but cargo accepts them.
                    // See https://github.com/steveklabnik/semver/issues/98.
                    if req == &*MAJOR_WILDCARD || req.matches(to_version) {
                        Ok(())
                    } else {
                        Err(Error::DepGraphInternalError(format!(
                            "{} -> {} ({}): version ({}) doesn't match requirement ({:?})",
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

    /// Returns information about the workspace for this metadata.
    pub fn workspace(&self) -> &Workspace {
        &self.workspace
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

    pub fn deps<'a>(
        &'a self,
        package_id: &PackageId,
    ) -> Option<impl Iterator<Item = DependencyInfo<'a>> + 'a> {
        self.deps_directed(package_id, Outgoing)
    }

    pub fn reverse_deps<'a>(
        &'a self,
        package_id: &PackageId,
    ) -> Option<impl Iterator<Item = DependencyInfo<'a>> + 'a> {
        self.deps_directed(package_id, Incoming)
    }

    fn deps_directed<'a>(
        &'a self,
        package_id: &PackageId,
        dir: Direction,
    ) -> Option<impl Iterator<Item = DependencyInfo<'a>> + 'a> {
        self.metadata(package_id)
            .map(|metadata| self.deps_node_idx_directed(metadata.node_idx, dir))
    }

    fn deps_node_idx_directed<'a>(
        &'a self,
        node_idx: NodeIndex<u32>,
        dir: Direction,
    ) -> impl Iterator<Item = DependencyInfo<'a>> + 'a {
        self.dep_graph
            .edges_directed(node_idx, dir)
            .map(move |edge| self.edge_to_dep(edge.source(), edge.target(), edge.weight()))
    }

    /// Keeps all edges that return true from the visit closure, and removes the others.
    ///
    /// The order edges are visited is not specified.
    pub fn retain_edges<F>(&mut self, visit: F)
    where
        F: Fn(DependencyInfo<'_>) -> bool,
    {
        let packages = &self.packages;
        self.dep_graph.retain_edges(|frozen_graph, edge_idx| {
            // This could use self.edge_to_dep for part of it but that that isn't compatible with
            // the borrow checker :(
            let (source, target) = frozen_graph
                .edge_endpoints(edge_idx)
                .expect("edge_idx should be valid");
            let from = &packages[&frozen_graph[source]];
            let to = &packages[&frozen_graph[target]];
            let edge = &frozen_graph[edge_idx];
            visit(DependencyInfo { from, to, edge })
        });
    }

    /// Returns all transitive dependencies for the given package IDs.
    pub fn transitive_deps<'a, 'b>(
        &'a self,
        package_ids: impl IntoIterator<Item = &'b PackageId>,
    ) -> Result<impl Iterator<Item = &'a PackageId> + 'a, Error> {
        let node_idxs = self.node_idxs(package_ids)?;

        let bfs = Bfs {
            stack: node_idxs,
            discovered: self.dep_graph.visit_map(),
        };

        Ok(bfs
            .iter(&self.dep_graph)
            .map(move |node_idx| &self.dep_graph[node_idx]))
    }

    /// Returns all transitive dependency edges for the given package IDs.
    pub fn transitive_dep_edges<'a, 'b>(
        &'a self,
        package_ids: impl IntoIterator<Item = &'b PackageId>,
    ) -> Result<impl Iterator<Item = DependencyInfo<'a>> + 'a, Error> {
        let node_idxs: Vec<_> = self.node_idxs(package_ids)?;
        let edge_bfs = EdgeBfs::new(&self.dep_graph, node_idxs);

        Ok(edge_bfs
            .iter(&self.dep_graph)
            .map(move |(source_idx, target_idx, edge_idx)| {
                self.edge_to_dep(source_idx, target_idx, &self.dep_graph[edge_idx])
            }))
    }

    // ---
    // Helper methods
    // ---

    fn dep_graph(&self) -> &Graph<PackageId, DependencyEdge> {
        &self.dep_graph
    }

    /// Maps an edge source, target and weight to a package dep.
    fn edge_to_dep<'a>(
        &'a self,
        source: NodeIndex<u32>,
        target: NodeIndex<u32>,
        edge: &'a DependencyEdge,
    ) -> DependencyInfo<'a> {
        // Note: It would be really lovely if this could just take in any EdgeRef with the right
        // parameters, but 'weight' wouldn't live long enough unfortunately.
        //
        // https://docs.rs/petgraph/0.4.13/petgraph/graph/struct.EdgeReference.html#method.weight
        // is defined separately for the same reason.
        let from = self
            .metadata(&self.dep_graph[source])
            .expect("'from' should have associated metadata");
        let to = self
            .metadata(&self.dep_graph[target])
            .expect("'to' should have associated metadata");
        DependencyInfo { from, to, edge }
    }

    /// Maps an iterator of package IDs to their internal graph node indexes.
    fn node_idxs<'a, 'b, B>(
        &'a self,
        package_ids: impl IntoIterator<Item = &'b PackageId>,
    ) -> Result<B, Error>
    where
        B: FromIterator<NodeIndex<u32>>,
    {
        package_ids
            .into_iter()
            .map(|package_id| {
                self.node_idx(package_id)
                    .ok_or_else(|| Error::DepGraphUnknownPackageId(package_id.clone()))
            })
            .collect()
    }

    /// Maps a package ID to its internal graph node index.
    fn node_idx(&self, package_id: &PackageId) -> Option<NodeIndex<u32>> {
        self.metadata(package_id).map(|metadata| metadata.node_idx)
    }
}

#[derive(Clone, Debug)]
pub struct Workspace {
    root: PathBuf,
    // This is a BTreeMap to allow presenting data in sorted order.
    members_by_path: BTreeMap<PathBuf, PackageId>,
}

impl Workspace {
    /// Returns the workspace root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns an iterator over of workspace paths and members, sorted by the path they're in.
    pub fn members(&self) -> impl Iterator<Item = (&Path, &PackageId)> + ExactSizeIterator {
        self.members_by_path
            .iter()
            .map(|(path, id)| (path.as_path(), id))
    }

    /// Maps the given path to the corresponding workspace member.
    pub fn member_by_path(&self, path: impl AsRef<Path>) -> Option<&PackageId> {
        self.members_by_path.get(path.as_ref())
    }
}

#[derive(Clone, Debug)]
pub struct DependencyInfo<'a> {
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
    manifest_path: PathBuf,

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

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
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

    /// Return true if this edge is dev-only, i.e. code from this edge will not be included in
    /// normal builds.
    pub fn dev_only(&self) -> bool {
        self.normal().is_none() && self.build.is_none()
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

fn kind_str(kind: DependencyKind) -> &'static str {
    match kind {
        DependencyKind::Normal => "normal",
        DependencyKind::Build => "build",
        DependencyKind::Development => "dev",
        _ => "unknown",
    }
}
