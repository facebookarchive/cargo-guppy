// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::feature::{FeatureGraphImpl, FeatureId, FeatureNode};
use crate::graph::{
    cargo_version_matches, BuildTarget, BuildTargetId, BuildTargetImpl, BuildTargetKind, Cycles,
    DependencyDirection, OwnedBuildTargetId, PackageIx,
};
use crate::petgraph_support::scc::Sccs;
use crate::{Error, JsonValue, Metadata, MetadataCommand, PackageId, Platform};
use cargo_metadata::{DependencyKind, NodeDep};
use fixedbitset::FixedBitSet;
use indexmap::IndexMap;
use once_cell::sync::OnceCell;
use petgraph::algo::{has_path_connecting, DfsSpace};
use petgraph::prelude::*;
use semver::{Version, VersionReq};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::iter;
use std::mem;
use std::path::{Path, PathBuf};
use target_spec::TargetSpec;

/// A graph of packages and dependencies between them, parsed from metadata returned by `cargo
/// metadata`.
///
/// For examples on how to use `PackageGraph`, see
/// [the `examples` directory](https://github.com/facebookincubator/cargo-guppy/tree/master/guppy/examples)
/// in this crate.
#[derive(Clone, Debug)]
pub struct PackageGraph {
    // Source of truth data.
    pub(super) dep_graph: Graph<PackageId, PackageEdgeImpl, Directed, PackageIx>,
    // The strongly connected components of the graph, computed on demand.
    pub(super) sccs: OnceCell<Sccs<PackageIx>>,
    // Feature graph, computed on demand.
    pub(super) feature_graph: OnceCell<FeatureGraphImpl>,
    // XXX Should this be in an Arc for quick cloning? Not clear how this would work with node
    // filters though.
    pub(super) data: PackageGraphData,
}

/// Per-package data for a PackageGraph instance.
#[derive(Clone, Debug)]
pub struct PackageGraphData {
    pub(super) packages: HashMap<PackageId, PackageMetadata>,
    pub(super) workspace: WorkspaceImpl,
}

impl PackageGraph {
    /// Constructs a package graph from the given command.
    pub fn from_command(command: &mut MetadataCommand) -> Result<Self, Error> {
        Self::new(command.exec().map_err(Error::CommandError)?)
    }

    /// Constructs a package graph from the given JSON output of `cargo metadata`.
    pub fn from_json(json: impl AsRef<str>) -> Result<Self, Error> {
        let metadata = serde_json::from_str(json.as_ref()).map_err(Error::MetadataParseError)?;
        Self::new(metadata)
    }

    /// Constructs a package graph from the given metadata.
    pub fn new(metadata: Metadata) -> Result<Self, Error> {
        Self::build(metadata)
    }

    /// Verifies internal invariants on this graph. Not part of the documented API.
    #[doc(hidden)]
    pub fn verify(&self) -> Result<(), Error> {
        // Graph structure checks.
        let node_count = self.dep_graph.node_count();
        let package_count = self.data.packages.len();
        if node_count != package_count {
            return Err(Error::PackageGraphInternalError(format!(
                "number of nodes = {} different from packages = {}",
                node_count, package_count,
            )));
        }

        // TODO: The dependency graph can have cyclic dev-dependencies. Add a check to ensure that
        // the graph without any dev-only dependencies is acyclic.

        let workspace = self.workspace();
        let workspace_ids: HashSet<_> = workspace.member_ids().collect();

        for metadata in self.packages() {
            let package_id = metadata.id();

            match metadata.workspace_path() {
                Some(workspace_path) => {
                    // This package is in the workspace, so the workspace should have information
                    // about it.
                    let metadata2 = workspace.member_by_path(workspace_path);
                    let metadata2_id = metadata2.map(|metadata| metadata.id());
                    if metadata2_id != Some(package_id) {
                        return Err(Error::PackageGraphInternalError(format!(
                            "package {} has workspace path {:?} but query by path returned {:?}",
                            package_id, workspace_path, metadata2_id,
                        )));
                    }

                    let metadata3 = workspace.member_by_name(metadata.name());
                    let metadata3_id = metadata3.map(|metadata| metadata.id());
                    if metadata3_id != Some(package_id) {
                        return Err(Error::PackageGraphInternalError(format!(
                            "package {} has name {}, but workspace query by name returned {:?}",
                            package_id,
                            metadata.name(),
                            metadata3_id,
                        )));
                    }
                }
                None => {
                    // This package is not in the workspace.
                    if workspace_ids.contains(package_id) {
                        return Err(Error::PackageGraphInternalError(format!(
                            "package {} has no workspace path but is in workspace",
                            package_id,
                        )));
                    }
                }
            }

            for build_target in metadata.build_targets() {
                match build_target.id() {
                    BuildTargetId::Library | BuildTargetId::BuildScript => {
                        // Ensure that the name is populated (this may panic if it isn't).
                        build_target.name();
                    }
                    BuildTargetId::Binary(name)
                    | BuildTargetId::Example(name)
                    | BuildTargetId::Test(name)
                    | BuildTargetId::Benchmark(name) => {
                        if name != build_target.name() {
                            return Err(Error::PackageGraphInternalError(format!(
                                "package {} has build target name mismatch ({} != {})",
                                package_id,
                                name,
                                build_target.name(),
                            )));
                        }
                    }
                }

                let id_kind_mismatch = match build_target.id() {
                    BuildTargetId::Library | BuildTargetId::Example(_) => {
                        match build_target.kind() {
                            BuildTargetKind::LibraryOrExample(_) => false,
                            BuildTargetKind::Binary => true,
                        }
                    }
                    BuildTargetId::BuildScript
                    | BuildTargetId::Binary(_)
                    | BuildTargetId::Test(_)
                    | BuildTargetId::Benchmark(_) => match build_target.kind() {
                        BuildTargetKind::LibraryOrExample(_) => true,
                        BuildTargetKind::Binary => false,
                    },
                };

                if id_kind_mismatch {
                    return Err(Error::PackageGraphInternalError(format!(
                        "package {} has build target id {:?}, which doesn't match kind {:?}",
                        package_id,
                        build_target.id(),
                        build_target.kind(),
                    )));
                }
            }

            for dep in self.dep_links_ixs_directed(metadata.package_ix, Outgoing) {
                let to_id = dep.to.id();
                let to_version = dep.to.version();

                // Two invariants:
                // 1. At least one of the edges should be specified.
                // 2. The specified package should match the version dependency.

                let req = dep.edge.version_req();
                // A requirement of "*" filters out pre-release versions with the semver crate,
                // but cargo accepts them.
                // See https://github.com/steveklabnik/semver/issues/98.
                if !cargo_version_matches(req, to_version) {
                    return Err(Error::PackageGraphInternalError(format!(
                        "{} -> {}: version ({}) doesn't match requirement ({:?})",
                        package_id, to_id, to_version, req,
                    )));
                }

                let edge_set = dep.edge.normal().is_some()
                    || dep.edge.build().is_some()
                    || dep.edge.dev().is_some();

                if !edge_set {
                    return Err(Error::PackageGraphInternalError(format!(
                        "{} -> {}: no edge info found",
                        package_id, to_id,
                    )));
                }
            }
        }

        // Constructing the feature graph may cause panics to happen.
        self.feature_graph();

        Ok(())
    }

    /// Returns information about the workspace.
    pub fn workspace(&self) -> Workspace {
        self.data.workspace()
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
    /// For an equivalent operation which doesn't mutate the graph, see
    /// `PackageQuery::resolve_with` and `PackageQuery::resolve_with_fn`.
    ///
    /// The order edges are visited is not specified.
    pub fn retain_edges<F>(&mut self, visit: F)
    where
        F: Fn(&PackageGraphData, PackageLink<'_>) -> bool,
    {
        let data = &self.data;
        self.dep_graph.retain_edges(|frozen_graph, edge_ix| {
            // This could use self.edge_to_dep for part of it but that that isn't compatible with
            // the borrow checker :(
            let (source, target) = frozen_graph
                .edge_endpoints(edge_ix)
                .expect("edge_ix should be valid");
            let from = &data.packages[&frozen_graph[source]];
            let to = &data.packages[&frozen_graph[target]];
            let edge = &frozen_graph[edge_ix];
            visit(
                data,
                PackageLink {
                    from,
                    to,
                    edge: PackageEdge {
                        edge_ix,
                        inner: edge,
                    },
                },
            )
        });

        self.invalidate_caches();
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
    /// This also returns true if `package_a` is the same as `package_b`.
    ///
    /// For repeated queries, consider using `new_depends_cache` to speed up queries.
    pub fn depends_on(&self, package_a: &PackageId, package_b: &PackageId) -> Result<bool, Error> {
        let mut depends_cache = self.new_depends_cache();
        depends_cache.depends_on(package_a, package_b)
    }

    /// Returns true if `package_a` directly depends on `package_b`.
    ///
    /// In other words, this returns true if `package_b` is a direct dependency of `package_a`.
    ///
    /// This returns false if `package_a` is the same as `package_b`.
    pub fn directly_depends_on(
        &self,
        package_a: &PackageId,
        package_b: &PackageId,
    ) -> Result<bool, Error> {
        let a_ix = self.package_ix_err(package_a)?;
        let b_ix = self.package_ix_err(package_b)?;
        Ok(self.dep_graph.contains_edge(a_ix, b_ix))
    }

    /// Returns information about dependency cycles in this graph.
    ///
    /// For more information, see the documentation for `Cycles`.
    pub fn cycles(&self) -> Cycles {
        Cycles::new(self)
    }

    // ---
    // Dependency traversals
    // ---

    /// Returns the direct dependencies for the given package ID in the specified direction.
    pub fn dep_links_directed<'g>(
        &'g self,
        package_id: &PackageId,
        dep_direction: DependencyDirection,
    ) -> Option<impl Iterator<Item = PackageLink<'g>> + 'g> {
        self.dep_links_impl(package_id, dep_direction.into())
    }

    /// Returns the direct dependencies for the given package ID.
    pub fn dep_links<'g>(
        &'g self,
        package_id: &PackageId,
    ) -> Option<impl Iterator<Item = PackageLink<'g>> + 'g> {
        self.dep_links_impl(package_id, Outgoing)
    }

    /// Returns the direct reverse dependencies for the given package ID.
    pub fn reverse_dep_links<'g>(
        &'g self,
        package_id: &PackageId,
    ) -> Option<impl Iterator<Item = PackageLink<'g>> + 'g> {
        self.dep_links_impl(package_id, Incoming)
    }

    fn dep_links_impl<'g>(
        &'g self,
        package_id: &PackageId,
        dir: Direction,
    ) -> Option<impl Iterator<Item = PackageLink<'g>> + 'g> {
        self.metadata(package_id)
            .map(|metadata| self.dep_links_ixs_directed(metadata.package_ix, dir))
    }

    fn dep_links_ixs_directed<'g>(
        &'g self,
        package_ix: NodeIndex<PackageIx>,
        dir: Direction,
    ) -> impl Iterator<Item = PackageLink<'g>> + 'g {
        self.dep_graph
            .edges_directed(package_ix, dir)
            .map(move |edge| {
                self.edge_to_link(edge.source(), edge.target(), edge.id(), Some(edge.weight()))
            })
    }

    // For more traversals, see query.rs.

    // ---
    // Helper methods
    // ---

    /// Constructs a map of strongly connected components for this graph.
    pub(super) fn sccs(&self) -> &Sccs<PackageIx> {
        self.sccs.get_or_init(|| Sccs::new(&self.dep_graph))
    }

    /// Invalidates internal caches. Meant to be called whenever the graph is mutated.
    pub(super) fn invalidate_caches(&mut self) {
        mem::replace(&mut self.sccs, OnceCell::new());
        mem::replace(&mut self.feature_graph, OnceCell::new());
    }

    /// Returns the inner dependency graph.
    ///
    /// Should this be exposed publicly? Not sure.
    pub(super) fn dep_graph(&self) -> &Graph<PackageId, PackageEdgeImpl, Directed, PackageIx> {
        &self.dep_graph
    }

    /// Maps an edge source, target and weight to a dependency link.
    pub(super) fn edge_to_link<'g>(
        &'g self,
        source: NodeIndex<PackageIx>,
        target: NodeIndex<PackageIx>,
        edge_ix: EdgeIndex<PackageIx>,
        edge: Option<&'g PackageEdgeImpl>,
    ) -> PackageLink<'g> {
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
        let edge = PackageEdge {
            edge_ix,
            inner: edge.unwrap_or_else(|| &self.dep_graph[edge_ix]),
        };
        PackageLink { from, to, edge }
    }

    /// Maps an iterator of package IDs to their internal graph node indexes.
    pub(super) fn package_ixs<'g, 'a, B>(
        &'g self,
        package_ids: impl IntoIterator<Item = &'a PackageId>,
    ) -> Result<B, Error>
    where
        B: iter::FromIterator<NodeIndex<PackageIx>>,
    {
        package_ids
            .into_iter()
            .map(|package_id| self.package_ix_err(package_id))
            .collect()
    }

    /// Maps a package ID to its internal graph node index.
    pub(super) fn package_ix(&self, package_id: &PackageId) -> Option<NodeIndex<PackageIx>> {
        self.metadata(package_id)
            .map(|metadata| metadata.package_ix)
    }

    /// Maps a package ID to its internal graph node index, and returns an `UnknownPackageId` error
    /// if the package isn't found.
    pub(super) fn package_ix_err(
        &self,
        package_id: &PackageId,
    ) -> Result<NodeIndex<PackageIx>, Error> {
        self.package_ix(package_id)
            .ok_or_else(|| Error::UnknownPackageId(package_id.clone()))
    }
}

impl PackageGraphData {
    /// Returns information about the workspace.
    pub fn workspace(&self) -> Workspace {
        Workspace {
            data: self,
            inner: &self.workspace,
        }
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
/// Created with `PackageGraph::new_depends_cache()`.
#[derive(Clone, Debug)]
pub struct DependsCache<'g> {
    package_graph: &'g PackageGraph,
    dfs_space: DfsSpace<NodeIndex<PackageIx>, FixedBitSet>,
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
        let a_ix = self.package_graph.package_ix_err(package_a)?;
        let b_ix = self.package_graph.package_ix_err(package_b)?;
        Ok(has_path_connecting(
            self.package_graph.dep_graph(),
            a_ix,
            b_ix,
            Some(&mut self.dfs_space),
        ))
    }
}

/// Information about a workspace, parsed from metadata returned by `cargo metadata`.
///
/// For more about workspaces, see
/// [Cargo Workspaces](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html) in *The Rust
/// Programming Language*.
#[derive(Clone, Debug)]
pub struct Workspace<'g> {
    data: &'g PackageGraphData,
    inner: &'g WorkspaceImpl,
}

impl<'g> Workspace<'g> {
    /// Returns the workspace root.
    pub fn root(&self) -> &'g Path {
        &self.inner.root
    }

    /// Returns an iterator over workspace paths and package metadatas, sorted by the path
    /// they're in.
    pub fn members(
        &self,
    ) -> impl Iterator<Item = (&'g Path, &'g PackageMetadata)> + ExactSizeIterator {
        let data = self.data;
        self.inner
            .members_by_path
            .iter()
            .map(move |(path, id)| (path.as_path(), data.metadata(id).expect("valid package ID")))
    }

    /// Returns an iterator over workspace names and package metadatas, sorted by names.
    pub fn members_by_name(
        &self,
    ) -> impl Iterator<Item = (&'g str, &'g PackageMetadata)> + ExactSizeIterator {
        let data = self.data;
        self.inner
            .members_by_name
            .iter()
            .map(move |(name, id)| (name.as_ref(), data.metadata(id).expect("valid package ID")))
    }

    /// Returns an iterator over package IDs for workspace members. The package IDs will be returned
    /// in the same order as `members`, sorted by the path they're in.
    pub fn member_ids(&self) -> impl Iterator<Item = &'g PackageId> + ExactSizeIterator {
        self.inner.members_by_path.iter().map(|(_path, id)| id)
    }

    /// Maps the given path to the corresponding workspace member.
    pub fn member_by_path(&self, path: impl AsRef<Path>) -> Option<&'g PackageMetadata> {
        let id = self.inner.members_by_path.get(path.as_ref())?;
        Some(self.data.metadata(id).expect("valid package ID"))
    }

    /// Maps the given name to the corresponding workspace member.
    pub fn member_by_name(&self, name: impl AsRef<str>) -> Option<&'g PackageMetadata> {
        let id = self.inner.members_by_name.get(name.as_ref())?;
        Some(self.data.metadata(id).expect("valid package ID"))
    }
}

#[derive(Clone, Debug)]
pub(super) struct WorkspaceImpl {
    pub(super) root: PathBuf,
    // This is a BTreeMap to allow presenting data in sorted order.
    pub(super) members_by_path: BTreeMap<PathBuf, PackageId>,
    pub(super) members_by_name: BTreeMap<Box<str>, PackageId>,
}

/// Represents a dependency from one package to another.
#[derive(Copy, Clone, Debug)]
pub struct PackageLink<'g> {
    /// The package which depends on the `to` package.
    pub from: &'g PackageMetadata,
    /// The package which is depended on by the `from` package.
    pub to: &'g PackageMetadata,
    /// Information about the specifics of this dependency.
    pub edge: PackageEdge<'g>,
}

/// Information about a specific package in a `PackageGraph`.
///
/// Most of the metadata is extracted from `Cargo.toml` files. See
/// [the `Cargo.toml` reference](https://doc.rust-lang.org/cargo/reference/manifest.html) for more
/// details.
#[derive(Clone, Debug)]
pub struct PackageMetadata {
    // Implementation note: we use Box<str> and Box<Path> to save on memory use when possible.

    // Fields extracted from the package.
    pub(super) id: PackageId,
    pub(super) name: String,
    pub(super) version: Version,
    pub(super) authors: Vec<String>,
    pub(super) description: Option<Box<str>>,
    pub(super) license: Option<Box<str>>,
    pub(super) license_file: Option<Box<Path>>,
    pub(super) manifest_path: Box<Path>,
    pub(super) categories: Vec<String>,
    pub(super) keywords: Vec<String>,
    pub(super) readme: Option<Box<Path>>,
    pub(super) repository: Option<Box<str>>,
    pub(super) edition: Box<str>,
    pub(super) metadata_table: JsonValue,
    pub(super) links: Option<Box<str>>,
    pub(super) publish: Option<Vec<String>>,
    // Some(...) means named feature with listed dependencies.
    // None means an optional dependency.
    pub(super) features: IndexMap<Box<str>, Option<Vec<String>>>,

    // Other information.
    pub(super) package_ix: NodeIndex<PackageIx>,
    pub(super) workspace_path: Option<Box<Path>>,
    pub(super) build_targets: BTreeMap<OwnedBuildTargetId, BuildTargetImpl>,
    pub(super) has_default_feature: bool,
    pub(super) resolved_deps: Vec<NodeDep>,
    pub(super) resolved_features: Vec<String>,
}

impl PackageMetadata {
    /// Returns the unique identifier for this package.
    pub fn id(&self) -> &PackageId {
        &self.id
    }

    /// Returns the name of this package.
    ///
    /// This is the same as the `name` field of `Cargo.toml`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the version of this package as resolved by Cargo.
    ///
    /// This is the same as the `version` field of `Cargo.toml`.
    pub fn version(&self) -> &Version {
        &self.version
    }

    /// Returns the authors of this package.
    ///
    /// This is the same as the `authors` field of `Cargo.toml`.
    pub fn authors(&self) -> &[String] {
        &self.authors
    }

    /// Returns a short description for this package.
    ///
    /// This is the same as the `description` field of `Cargo.toml`.
    pub fn description(&self) -> Option<&str> {
        self.description.as_ref().map(|x| x.as_ref())
    }

    /// Returns an SPDX 2.1 license expression for this package, if specified.
    ///
    /// This is the same as the `license` field of `Cargo.toml`. Note that `guppy` does not perform
    /// any validation on this, though `crates.io` does if a crate is uploaded there.
    pub fn license(&self) -> Option<&str> {
        self.license.as_ref().map(|x| x.as_ref())
    }

    /// Returns the path to a license file for this package, if specified.
    ///
    /// This is the same as the `license_file` field of `Cargo.toml`. It is typically only specified
    /// for nonstandard licenses.
    pub fn license_file(&self) -> Option<&Path> {
        self.license_file.as_ref().map(|path| path.as_ref())
    }

    /// Returns the full path to the `Cargo.toml` for this package.
    ///
    /// This is specific to the system that `cargo metadata` was run on.
    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    /// Returns categories for this package.
    ///
    /// This is the same as the `categories` field of `Cargo.toml`. For packages on `crates.io`,
    /// returned values are guaranteed to be
    /// [valid category slugs](https://crates.io/category_slugs).
    pub fn categories(&self) -> &[String] {
        &self.categories
    }

    /// Returns keywords for this package.
    ///
    /// This is the same as the `keywords` field of `Cargo.toml`.
    pub fn keywords(&self) -> &[String] {
        &self.keywords
    }

    /// Returns a path to the README for this package, if specified.
    ///
    /// This is the same as the `readme` field of `Cargo.toml`. The path returned is relative to the
    /// directory the `Cargo.toml` is in (i.e. relative to the parent of `self.manifest_path()`).
    pub fn readme(&self) -> Option<&Path> {
        self.readme.as_ref().map(|path| path.as_ref())
    }

    /// Returns the source code repository for this package, if specified.
    ///
    /// This is the same as the `repository` field of `Cargo.toml`.
    pub fn repository(&self) -> Option<&str> {
        self.repository.as_ref().map(|x| x.as_ref())
    }

    /// Returns the Rust edition this package is written against.
    ///
    /// This is the same as the `edition` field of `Cargo.toml`. It is `"2015"` by default.
    pub fn edition(&self) -> &str {
        &self.edition
    }

    /// Returns the freeform metadata table for this package.
    ///
    /// This is the same as the `package.metadata` section of `Cargo.toml`. This section is
    /// typically used by tools which would like to store package configuration in `Cargo.toml`.
    pub fn metadata_table(&self) -> &JsonValue {
        &self.metadata_table
    }

    /// Returns the name of a native library this package links to, if specified.
    ///
    /// This is the same as the `links` field of `Cargo.toml`. See [The `links` Manifest
    /// Key](https://doc.rust-lang.org/cargo/reference/build-scripts.html#the-links-manifest-key) in
    /// the Cargo book for more details.
    pub fn links(&self) -> Option<&str> {
        self.links.as_ref().map(|x| x.as_ref())
    }

    /// Returns the list of registries to which this package may be published.
    ///
    /// Returns `None` if publishing is unrestricted, and `Some(&[])` if publishing is forbidden.
    ///
    /// This is the same as the `publish` field of `Cargo.toml`.
    pub fn publish(&self) -> Option<&[String]> {
        self.publish.as_deref()
    }

    /// Returns true if this package is in the workspace.
    pub fn in_workspace(&self) -> bool {
        self.workspace_path.is_some()
    }

    /// Returns the relative path to this package in the workspace, or `None` if this package is
    /// not in the workspace.
    pub fn workspace_path(&self) -> Option<&Path> {
        self.workspace_path.as_ref().map(|path| path.as_ref())
    }

    /// Returns all the build targets for this package.
    ///
    /// For more, see [Cargo
    /// Targets](https://doc.rust-lang.org/nightly/cargo/reference/cargo-targets.html#cargo-targets)
    /// in the Cargo reference.
    pub fn build_targets(&self) -> impl Iterator<Item = BuildTarget> {
        self.build_targets.iter().map(BuildTarget::new)
    }

    /// Looks up a build target by identifier.
    pub fn build_target(&self, id: &BuildTargetId<'_>) -> Option<BuildTarget> {
        self.build_targets
            .get_key_value(id.as_key())
            .map(BuildTarget::new)
    }

    /// Returns true if this package has a named feature named `default`.
    ///
    /// For more about default features, see [The `[features]`
    /// section](https://doc.rust-lang.org/cargo/reference/manifest.html#the-features-section) in
    /// the Cargo reference.
    pub fn has_default_feature(&self) -> bool {
        self.has_default_feature
    }

    /// Returns the `FeatureId` corresponding to the default feature.
    pub fn default_feature_id(&self) -> FeatureId {
        if self.has_default_feature {
            FeatureId::new(self.id(), "default")
        } else {
            FeatureId::base(self.id())
        }
    }

    /// Returns the list of named features available for this package. This will include a feature
    /// named "default" if it is defined.
    ///
    /// A named feature is listed in the `[features]` section of `Cargo.toml`. For more, see
    /// [the reference](https://doc.rust-lang.org/cargo/reference/manifest.html#the-features-section).
    pub fn named_features(&self) -> impl Iterator<Item = &str> {
        self.named_features_full()
            .map(|(_, named_feature, _)| named_feature)
    }

    // ---
    // Helper methods
    // --

    pub(super) fn get_feature_idx(&self, feature: &str) -> Option<usize> {
        self.features.get_full(feature).map(|(n, _, _)| n)
    }

    #[allow(dead_code)]
    pub(super) fn all_feature_nodes<'g>(&'g self) -> impl Iterator<Item = FeatureNode> + 'g {
        iter::once(FeatureNode::base(self.package_ix)).chain(
            (0..self.features.len())
                .map(move |feature_idx| FeatureNode::new(self.package_ix, feature_idx)),
        )
    }

    pub(super) fn named_features_full(&self) -> impl Iterator<Item = (usize, &str, &[String])> {
        self.features
            .iter()
            // IndexMap is documented to use indexes 0..n without holes, so this enumerate()
            // is correct.
            .enumerate()
            .filter_map(|(n, (feature, deps))| {
                deps.as_ref()
                    .map(|deps| (n, feature.as_ref(), deps.as_slice()))
            })
    }

    pub(super) fn optional_deps_full(&self) -> impl Iterator<Item = (usize, &str)> {
        self.features
            .iter()
            // IndexMap is documented to use indexes 0..n without holes, so this enumerate()
            // is correct.
            .enumerate()
            .filter_map(|(n, (feature, deps))| {
                if deps.is_none() {
                    Some((n, feature.as_ref()))
                } else {
                    None
                }
            })
    }
}

/// Details about a specific dependency from a package to another package.
///
/// Usually found within the context of a [`PackageLink`](struct.PackageLink.html).
///
/// This struct contains information about:
/// * whether this dependency was renamed in the context of this crate.
/// * if this is a normal, dev or build dependency.
#[derive(Copy, Clone, Debug)]
pub struct PackageEdge<'g> {
    edge_ix: EdgeIndex<PackageIx>,
    inner: &'g PackageEdgeImpl,
}

impl<'g> PackageEdge<'g> {
    /// Returns the name for this dependency edge. This can be affected by a crate rename.
    pub fn dep_name(&self) -> &'g str {
        &self.inner.dep_name
    }

    /// Returns the resolved name for this dependency edge. This may involve renaming the crate and
    /// replacing - with _.
    pub fn resolved_name(&self) -> &'g str {
        &self.inner.resolved_name
    }

    /// Returns the semver requirements specified for this dependency.
    ///
    /// To get the resolved version, see the `to` field of the `PackageLink` this was part of.
    ///
    /// ## Notes
    ///
    /// A dependency can be requested multiple times, possibly with different version requirements,
    /// even if they all end up resolving to the same version. `version_req` will return any of
    /// those requirements.
    ///
    /// See [Specifying Dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#specifying-dependencies)
    /// in the Cargo reference for more details.
    pub fn version_req(&self) -> &'g VersionReq {
        &self.inner.version_req
    }

    /// Returns details about this dependency from the `[dependencies]` section, if they exist.
    pub fn normal(&self) -> Option<&'g DependencyMetadata> {
        self.inner.normal.as_ref()
    }

    /// Returns details about this dependency from the `[build-dependencies]` section, if they exist.
    pub fn build(&self) -> Option<&'g DependencyMetadata> {
        self.inner.build.as_ref()
    }

    /// Returns details about this dependency from the `[dev-dependencies]` section, if they exist.
    pub fn dev(&self) -> Option<&'g DependencyMetadata> {
        // XXX should dev dependencies fall back to normal if no dev-specific data was found?
        self.inner.dev.as_ref()
    }

    /// Returns details about this dependency from the section specified by the given dependency
    /// kind.
    pub fn metadata_for_kind(&self, kind: DependencyKind) -> Option<&'g DependencyMetadata> {
        match kind {
            DependencyKind::Normal => self.normal(),
            DependencyKind::Development => self.dev(),
            DependencyKind::Build => self.build(),
            _ => panic!("dependency metadata requested for unknown kind: {:?}", kind),
        }
    }

    /// Return true if this edge is dev-only, i.e. code from this edge will not be included in
    /// normal builds.
    pub fn dev_only(&self) -> bool {
        self.normal().is_none() && self.build().is_none()
    }

    // ---
    // Helper methods
    // ---

    /// Returns the edge index.
    #[allow(dead_code)]
    pub(super) fn edge_ix(&self) -> EdgeIndex<PackageIx> {
        self.edge_ix
    }

    /// Returns the inner `PackageEdgeImpl` as a pointer. Useful for testing.
    #[cfg(test)]
    pub(crate) fn as_inner_ptr(&self) -> *const PackageEdgeImpl {
        self.inner
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PackageEdgeImpl {
    pub(super) dep_name: String,
    pub(super) resolved_name: String,
    pub(super) version_req: VersionReq,
    pub(super) normal: Option<DependencyMetadata>,
    pub(super) build: Option<DependencyMetadata>,
    pub(super) dev: Option<DependencyMetadata>,
}

/// Information about a specific kind of dependency (normal, build or dev) from a package to another
/// package.
///
/// Usually found within the context of a [`PackageEdge`](struct.PackageEdge.html).
#[derive(Clone, Debug)]
pub struct DependencyMetadata {
    pub(super) dependency_req: DependencyReq,

    // Results of some queries as evaluated on the current platform.
    pub(super) current_enabled: EnabledStatus,
    pub(super) current_default_features: EnabledStatus,
    pub(super) all_features: Vec<String>,
    pub(super) current_feature_statuses: HashMap<String, EnabledStatus>,
}

impl DependencyMetadata {
    /// Returns true if this is an optional dependency on the platform `guppy` is running on.
    ///
    /// This will also return true if this dependency will never be included on this platform at
    /// all, or if the status is unknown. To get finer-grained information, use the `enabled` method
    /// instead.
    pub fn optional(&self) -> bool {
        self.current_enabled != EnabledStatus::Always
    }

    /// Returns true if this is an optional dependency on the given platform.
    ///
    /// This will also return true if this dependency will never be included on this platform at
    /// all, or if the status is unknown. To get finer-grained information, use the `enabled_on`
    /// method instead.
    pub fn optional_on(&self, platform: &Platform<'_>) -> bool {
        self.dependency_req.enabled_on(platform) != EnabledStatus::Always
    }

    /// Returns the enabled status of this dependency on the platform `guppy` is running on.
    ///
    /// See the documentation for `EnabledStatus` for more.
    pub fn enabled(&self) -> EnabledStatus {
        self.current_enabled
    }

    /// Returns the enabled status of this dependency on the given platform.
    ///
    /// See the documentation for `EnabledStatus` for more.
    pub fn enabled_on(&self, platform: &Platform<'_>) -> EnabledStatus {
        self.dependency_req.enabled_on(platform)
    }

    /// Returns true if the default features of this dependency are enabled on the platform `guppy`
    /// is running on.
    ///
    /// It is possible for default features to be turned off by default, but be optionally included.
    /// This method returns true in those cases. To get finer-grained information, use
    /// the `default_features` method instead.
    pub fn uses_default_features(&self) -> bool {
        self.current_default_features != EnabledStatus::Never
    }

    /// Returns the status of default features on the platform `guppy` is running on.
    ///
    /// See the documentation for `EnabledStatus` for more.
    pub fn default_features(&self) -> EnabledStatus {
        self.current_default_features
    }

    /// Returns the status of default features of this dependency on the given platform.
    ///
    /// See the documentation for `EnabledStatus` for more.
    pub fn default_features_on(&self, platform: &Platform<'_>) -> EnabledStatus {
        self.dependency_req.default_features_on(platform)
    }

    /// Returns a list of all features possibly enabled by this dependency. This includes features
    /// that are only turned on if the dependency is optional, or features enabled by inactive
    /// platforms.
    pub fn features(&self) -> &[String] {
        &self.all_features
    }

    /// Returns the enabled status of the feature on the platform `guppy` is running on.
    ///
    /// Note that as of Rust 1.42, the default feature resolver behaves in potentially surprising
    /// ways. See the [Cargo
    /// reference](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#features) for
    /// more.
    ///
    /// See the documentation for `EnabledStatus` for more.
    pub fn feature_enabled(&self, feature: &str) -> EnabledStatus {
        self.current_feature_statuses
            .get(feature)
            .copied()
            .unwrap_or(EnabledStatus::Never)
    }

    /// Returns the enabled status of the feature on the given platform.
    ///
    /// See the documentation of `EnabledStatus` for more.
    pub fn feature_enabled_on(&self, feature: &str, platform: &Platform<'_>) -> EnabledStatus {
        self.dependency_req.feature_enabled_on(feature, platform)
    }
}

/// Whether a dependency or feature is enabled on a specific platform.
///
/// Returned by the methods on `DependencyMetadata`.
///
/// ## Examples
///
/// ```toml
/// [dependencies]
/// once_cell = "1"
/// ```
///
/// The dependency and default features are *always* enabled on all platforms.
///
/// ```toml
/// [dependencies]
/// once_cell = { version = "1", optional = true }
/// ```
///
/// The dependency and default features are *optional* on all platforms.
///
/// ```toml
/// [target.'cfg(windows)'.dependencies]
/// once_cell = { version = "1", optional = true }
/// ```
///
/// On Windows, the dependency and default features are both *optional*. On non-Windows platforms,
/// the dependency and default features are *never* enabled.
///
/// ```toml
/// [dependencies]
/// once_cell = { version = "1", optional = true }
///
/// [target.'cfg(windows)'.dependencies]
/// once_cell = { version = "1", optional = false, default-features = false }
/// ```
///
/// On Windows, the dependency is *always* enabled and default features are *optional* (i.e. enabled
/// if the `once_cell` feature is turned on).
///
/// On Unix platforms, the dependency and default features are both *optional*.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EnabledStatus {
    /// This dependency or feature is always enabled on this platform.
    Always,
    /// This dependency or feature is optionally enabled on this platform.
    Optional,
    /// This dependency or feature is never enabled on this platform, even if the optional
    /// dependency is turned on.
    Never,
    /// The status of this dependency is *unknown* because the evaluation involved target features
    /// whose status is unknown.
    ///
    /// This can only be returned if the set of target features is unknown. In particular, it is
    /// guaranteed never to be returned for queries being evaluated against the current platform,
    /// since the exact set of target features is already known.
    Unknown(UnknownStatus),
}

impl EnabledStatus {
    /// Converts a required evaluation result and a thunk returning the optional result into an
    /// `EnabledStatus`.
    pub(super) fn new(
        required_res: Option<bool>,
        optional_res_fn: impl FnOnce() -> Option<bool>,
    ) -> Self {
        //    required     optional      |      result
        //                               |
        //        T            *         |      always
        //        U            T         |  optional present
        //        U            U         |      unknown   [1]
        //        U            F         |      unknown   [1]
        //        F            T         |      optional
        //        F            U         |  optional unknown
        //        F            F         |       never
        //
        // [1] note that both these cases are collapsed into "unknown" -- for either of these it's
        //     not known whether the dependency will be included at all.

        match required_res {
            Some(true) => EnabledStatus::Always,
            None => match optional_res_fn() {
                Some(true) => EnabledStatus::Unknown(UnknownStatus::OptionalPresent),
                None | Some(false) => EnabledStatus::Unknown(UnknownStatus::Unknown),
            },
            Some(false) => match optional_res_fn() {
                Some(true) => EnabledStatus::Optional,
                None => EnabledStatus::Unknown(UnknownStatus::OptionalUnknown),
                Some(false) => EnabledStatus::Never,
            },
        }
    }

    /// Returns true if the enabled status is not `Unknown`.
    pub fn is_known(self) -> bool {
        match self {
            EnabledStatus::Always | EnabledStatus::Optional | EnabledStatus::Never => true,
            EnabledStatus::Unknown(_) => false,
        }
    }
}

/// More information about a dependency or feature whose evaluation is unknown.
///
/// If the result of evaluating a dependency or feature is unknown, this enum specifies why.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum UnknownStatus {
    /// Whether this dependency or feature is present by default or optionally is unknown.
    Unknown,
    /// This dependency or feature is present optionally, but whether it is included by default is
    /// unknown.
    OptionalPresent,
    /// This dependency or feature is not present by default, and whether it is included optionally
    /// is unknown.
    OptionalUnknown,
}

/// Information about dependency requirements.
#[derive(Clone, Debug, Default)]
pub(super) struct DependencyReq {
    pub(super) required: DependencyReqImpl,
    pub(super) optional: DependencyReqImpl,
}

impl DependencyReq {
    pub(super) fn enabled_on(&self, platform: &Platform<'_>) -> EnabledStatus {
        self.eval(|req_impl| &req_impl.build_if, platform)
    }

    pub(super) fn default_features_on(&self, platform: &Platform<'_>) -> EnabledStatus {
        self.eval(|req_impl| &req_impl.default_features_if, platform)
    }

    fn eval(
        &self,
        pred_fn: impl Fn(&DependencyReqImpl) -> &TargetPredicate,
        platform: &Platform<'_>,
    ) -> EnabledStatus {
        let required_res = pred_fn(&self.required).eval(platform);
        EnabledStatus::new(required_res, || pred_fn(&self.optional).eval(platform))
    }

    pub(super) fn feature_enabled_on(
        &self,
        feature: &str,
        platform: &Platform<'_>,
    ) -> EnabledStatus {
        let matches = move |req: &DependencyReqImpl| {
            let mut res = Some(false);
            for (target, features) in &req.target_features {
                if !features.iter().any(|f| f == feature) {
                    continue;
                }
                let target_matches = match target {
                    Some(spec) => spec.eval(platform),
                    None => Some(true),
                };
                // Short-circuit evaluation if possible.
                if target_matches == Some(true) {
                    return Some(true);
                }
                res = k3_or(res, target_matches);
            }
            res
        };

        EnabledStatus::new(matches(&self.required), || matches(&self.optional))
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct DependencyReqImpl {
    pub(super) build_if: TargetPredicate,
    pub(super) default_features_if: TargetPredicate,
    pub(super) target_features: Vec<(Option<TargetSpec>, Vec<String>)>,
}

impl DependencyReqImpl {
    pub(super) fn all_features(&self) -> impl Iterator<Item = &str> {
        self.target_features
            .iter()
            .flat_map(|(_, features)| features)
            .map(|s| s.as_str())
    }
}

#[derive(Clone, Debug)]
pub(crate) enum TargetPredicate {
    Always,
    // Empty vector means never.
    Specs(Vec<TargetSpec>),
}

impl TargetPredicate {
    /// Returns true if this is an empty predicate (i.e. will never match).
    pub(super) fn is_never(&self) -> bool {
        match self {
            TargetPredicate::Always => false,
            TargetPredicate::Specs(specs) => specs.is_empty(),
        }
    }

    /// Evaluates this target against the given platform triple.
    pub(super) fn eval(&self, platform: &Platform<'_>) -> Option<bool> {
        match self {
            TargetPredicate::Always => Some(true),
            TargetPredicate::Specs(specs) => {
                let mut res = Some(false);
                for spec in specs.iter() {
                    let matches = spec.eval(platform);
                    // Short-circuit evaluation if possible.
                    if matches == Some(true) {
                        return Some(true);
                    }
                    res = k3_or(res, matches);
                }
                res
            }
        }
    }
}

/// The OR operation for a 3-valued logic with true, false and unknown. One example of this is the
/// Kleene K3 logic.
fn k3_or(a: Option<bool>, b: Option<bool>) -> Option<bool> {
    match (a, b) {
        (Some(false), Some(false)) => Some(false),
        (Some(true), _) | (_, Some(true)) => Some(true),
        _ => None,
    }
}
