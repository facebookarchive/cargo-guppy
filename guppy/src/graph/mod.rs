use crate::errors::Error;
use cargo_metadata::{Dependency, DependencyKind, Metadata, MetadataCommand, NodeDep, PackageId};
use lazy_static::lazy_static;
use petgraph::algo::{has_path_connecting, toposort, DfsSpace};
use petgraph::prelude::*;
use petgraph::visit::{IntoNeighborsDirected, IntoNodeIdentifiers, Visitable};
use semver::{Version, VersionReq};
use std::collections::{BTreeMap, HashMap};
use std::iter;
use std::path::{Path, PathBuf};

mod build;
mod print;
mod select;

// Public exports for dot graphs.
pub use crate::petgraph_support::dot::DotWrite;
pub use print::PackageDotVisitor;
pub use select::{DependencyLinkIter, PackageIdIter, PackageSelect};

/// The direction in which to follow dependencies.
///
/// Used by the `_directed` methods.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DependencyDirection {
    /// Dependencies from this package to other packages.
    Forward,
    /// Reverse dependencies from other packages to this one.
    Reverse,
}

impl DependencyDirection {
    /// Returns the opposite direction to this one.
    pub fn opposite(&self) -> Self {
        match self {
            DependencyDirection::Forward => DependencyDirection::Reverse,
            DependencyDirection::Reverse => DependencyDirection::Forward,
        }
    }

    fn to_direction(self) -> Direction {
        match self {
            DependencyDirection::Forward => Direction::Outgoing,
            DependencyDirection::Reverse => Direction::Incoming,
        }
    }
}

/// A graph of packages extracted from a metadata.
#[derive(Clone, Debug)]
pub struct PackageGraph {
    // Source of truth data.
    dep_graph: Graph<PackageId, DependencyEdge>,
    // XXX Should this be in an Arc for quick cloning? Not clear how this would work with node
    // filters though.
    data: PackageGraphData,
}

/// Per-package data for a PackageGraph instance.
#[derive(Clone, Debug)]
pub struct PackageGraphData {
    packages: HashMap<PackageId, PackageMetadata>,
    workspace: Workspace,
}

impl PackageGraph {
    /// Constructs a package graph from the given command.
    pub fn from_command(command: &mut MetadataCommand) -> Result<Self, Error> {
        Self::new(command.exec().map_err(Error::CommandError)?)
    }

    /// Constructs a package graph from the given metadata.
    pub fn new(metadata: Metadata) -> Result<Self, Error> {
        Self::build(metadata)
    }

    /// Verifies internal invariants on this graph. Not part of the documented API.
    #[doc(hidden)]
    pub fn verify(&self) -> Result<(), Error> {
        lazy_static! {
            static ref MAJOR_WILDCARD: VersionReq = VersionReq::parse("*").unwrap();
        }

        // Graph structure checks.
        let node_count = self.dep_graph.node_count();
        let package_count = self.data.packages.len();
        if node_count != package_count {
            return Err(Error::DepGraphInternalError(format!(
                "number of nodes = {} different from packages = {}",
                node_count, package_count,
            )));
        }
        // petgraph has both is_cyclic_directed and toposort to detect cycles. is_cyclic_directed
        // is recursive and toposort is iterative. Package graphs have unbounded depth so use the
        // iterative implementation.
        if let Err(cycle) = toposort(&self.dep_graph, None) {
            return Err(Error::DepGraphInternalError(format!(
                "unexpected cycle in dep graph: {:?}",
                cycle
            )));
        }

        for metadata in self.packages() {
            let package_id = metadata.id();
            for dep in self.dep_links_node_idx_directed(metadata.node_idx, Outgoing) {
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

    /// Returns information about the workspace.
    pub fn workspace(&self) -> &Workspace {
        &self.data.workspace()
    }

    /// Returns an iterator over all the package IDs in this graph.
    pub fn package_ids(&self) -> impl Iterator<Item = &PackageId> + ExactSizeIterator {
        self.data.package_ids()
    }

    /// Returns an iterator over all the packages in this graph.
    pub fn packages(&self) -> impl Iterator<Item = &PackageMetadata> + ExactSizeIterator {
        self.data.packages()
    }

    /// Returns the number of packages in this graph.
    pub fn package_count(&self) -> usize {
        // This can be obtained in two different ways: self.dep_graph.node_count() or
        // self.data.packages.len(). verify() checks that they return the same results.
        //
        // Use this way for symmetry with link_count below (which can only be obtained through the
        // graph).
        self.dep_graph.node_count()
    }

    /// Returns the number of links in this graph.
    pub fn link_count(&self) -> usize {
        self.dep_graph.edge_count()
    }

    /// Returns the metadata for the given package ID.
    pub fn metadata(&self, package_id: &PackageId) -> Option<&PackageMetadata> {
        self.data.metadata(package_id)
    }

    /// Keeps all edges that return true from the visit closure, and removes the others.
    ///
    /// The order edges are visited is not specified.
    pub fn retain_edges<F>(&mut self, visit: F)
    where
        F: Fn(&PackageGraphData, DependencyLink<'_>) -> bool,
    {
        let data = &self.data;
        self.dep_graph.retain_edges(|frozen_graph, edge_idx| {
            // This could use self.edge_to_dep for part of it but that that isn't compatible with
            // the borrow checker :(
            let (source, target) = frozen_graph
                .edge_endpoints(edge_idx)
                .expect("edge_idx should be valid");
            let from = &data.packages[&frozen_graph[source]];
            let to = &data.packages[&frozen_graph[target]];
            let edge = &frozen_graph[edge_idx];
            visit(data, DependencyLink { from, to, edge })
        });
    }

    /// Creates a new cache for `depends_on` queries.
    ///
    /// The cache is optional but can speed up some queries.
    pub fn new_depends_cache(&self) -> DependsCache {
        DependsCache::new(self)
    }

    /// Returns true if `package_a` depends (directly or indirectly) on `package_b`.
    ///
    /// In other words, this returns true if `package_b` is a (possibly transitive) dependency of
    /// `package_a`.
    ///
    /// For repeated queries, consider using `new_depends_cache` to speed up queries.
    pub fn depends_on(&self, package_a: &PackageId, package_b: &PackageId) -> Result<bool, Error> {
        let mut depends_cache = self.new_depends_cache();
        depends_cache.depends_on(package_a, package_b)
    }

    // ---
    // Dependency traversals
    // ---

    /// Returns the direct dependencies for the given package ID in the specified direction.
    pub fn dep_links_directed<'g>(
        &'g self,
        package_id: &PackageId,
        dep_direction: DependencyDirection,
    ) -> Option<impl Iterator<Item = DependencyLink<'g>> + 'g> {
        self.dep_links_impl(package_id, dep_direction.to_direction())
    }

    /// Returns the direct dependencies for the given package ID.
    pub fn dep_links<'g>(
        &'g self,
        package_id: &PackageId,
    ) -> Option<impl Iterator<Item = DependencyLink<'g>> + 'g> {
        self.dep_links_impl(package_id, Outgoing)
    }

    /// Returns the direct reverse dependencies for the given package ID.
    pub fn reverse_dep_links<'g>(
        &'g self,
        package_id: &PackageId,
    ) -> Option<impl Iterator<Item = DependencyLink<'g>> + 'g> {
        self.dep_links_impl(package_id, Incoming)
    }

    fn dep_links_impl<'g>(
        &'g self,
        package_id: &PackageId,
        dir: Direction,
    ) -> Option<impl Iterator<Item = DependencyLink<'g>> + 'g> {
        self.metadata(package_id)
            .map(|metadata| self.dep_links_node_idx_directed(metadata.node_idx, dir))
    }

    fn dep_links_node_idx_directed<'g>(
        &'g self,
        node_idx: NodeIndex<u32>,
        dir: Direction,
    ) -> impl Iterator<Item = DependencyLink<'g>> + 'g {
        self.dep_graph
            .edges_directed(node_idx, dir)
            .map(move |edge| self.edge_to_link(edge.source(), edge.target(), edge.weight()))
    }

    // For more traversals, see select.rs.

    // ---
    // Helper methods
    // ---

    /// Returns the nodes of a graph that have no incoming edges to them.
    fn roots<G, B>(graph: G) -> B
    where
        G: IntoNodeIdentifiers + IntoNeighborsDirected<NodeId = NodeIndex<u32>>,
        B: iter::FromIterator<NodeIndex<u32>>,
    {
        graph
            .node_identifiers()
            .filter(move |&a| graph.neighbors_directed(a, Incoming).next().is_none())
            .collect()
    }

    /// Maps an edge source, target and weight to a dependency link.
    fn edge_to_link<'g>(
        &'g self,
        source: NodeIndex<u32>,
        target: NodeIndex<u32>,
        edge: &'g DependencyEdge,
    ) -> DependencyLink<'g> {
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
        DependencyLink { from, to, edge }
    }

    /// Maps an iterator of package IDs to their internal graph node indexes.
    fn node_idxs<'g, 'a, B>(
        &'g self,
        package_ids: impl IntoIterator<Item = &'a PackageId>,
    ) -> Result<B, Error>
    where
        B: iter::FromIterator<NodeIndex<u32>>,
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

impl PackageGraphData {
    /// Returns information about the workspace.
    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    /// Returns an iterator over all the package IDs in this graph.
    pub fn package_ids(&self) -> impl Iterator<Item = &PackageId> + ExactSizeIterator {
        self.packages.keys()
    }

    /// Returns an iterator over all the packages in this graph.
    pub fn packages(&self) -> impl Iterator<Item = &PackageMetadata> + ExactSizeIterator {
        self.packages.values()
    }

    /// Returns the metadata for the given package ID.
    pub fn metadata(&self, package_id: &PackageId) -> Option<&PackageMetadata> {
        self.packages.get(package_id)
    }
}

/// An optional cache used to speed up `depends_on` queries.
///
/// Created with `PackageGraph::new_cache()`.
#[derive(Clone, Debug)]
pub struct DependsCache<'g> {
    package_graph: &'g PackageGraph,
    dfs_space: DfsSpace<NodeIndex<u32>, <Graph<NodeIndex<u32>, EdgeIndex<u32>> as Visitable>::Map>,
}

impl<'g> DependsCache<'g> {
    /// Creates a new cache for `depends_on` queries for this package graph.
    ///
    /// This holds a shared reference to the package graph. This is to ensure that the cache is
    /// invalidated if the package graph is mutated.
    pub fn new(package_graph: &'g PackageGraph) -> Self {
        Self {
            package_graph,
            dfs_space: DfsSpace::new(&package_graph.dep_graph),
        }
    }

    /// Returns true if `package_a` depends (directly or indirectly) on `package_b`.
    ///
    /// In other words, this returns true if `package_b` is a (possibly transitive) dependency of
    /// `package_a`.
    pub fn depends_on(
        &mut self,
        package_a: &PackageId,
        package_b: &PackageId,
    ) -> Result<bool, Error> {
        // XXX rewrite this to avoid an allocation? meh
        let node_idxs: Vec<_> = self
            .package_graph
            .node_idxs(iter::once(package_a).chain(iter::once(package_b)))?;
        Ok(has_path_connecting(
            &self.package_graph.dep_graph,
            node_idxs[0],
            node_idxs[1],
            Some(&mut self.dfs_space),
        ))
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

#[derive(Copy, Clone, Debug)]
pub struct DependencyLink<'g> {
    pub from: &'g PackageMetadata,
    pub to: &'g PackageMetadata,
    pub edge: &'g DependencyEdge,
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
