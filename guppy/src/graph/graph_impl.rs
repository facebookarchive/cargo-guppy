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
    pub(super) dep_graph: Graph<PackageId, PackageLinkImpl, Directed, PackageIx>,
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
    pub(super) packages: HashMap<PackageId, PackageMetadataImpl>,
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

    /// Constructs a package graph from the given Cargo metadata, represented as a `Metadata`.
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
                    BuildTargetId::Library => match build_target.kind() {
                        BuildTargetKind::LibraryOrExample(_) | BuildTargetKind::ProcMacro => false,
                        BuildTargetKind::Binary => true,
                    },
                    BuildTargetId::Example(_) => match build_target.kind() {
                        BuildTargetKind::LibraryOrExample(_) => false,
                        BuildTargetKind::ProcMacro | BuildTargetKind::Binary => true,
                    },
                    BuildTargetId::BuildScript
                    | BuildTargetId::Binary(_)
                    | BuildTargetId::Test(_)
                    | BuildTargetId::Benchmark(_) => match build_target.kind() {
                        BuildTargetKind::LibraryOrExample(_) | BuildTargetKind::ProcMacro => true,
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

            for link in self.dep_links_ixs_directed(metadata.package_ix(), Outgoing) {
                let to = link.to();
                let to_id = to.id();
                let to_version = to.version();

                // Two invariants:
                // 1. At least one of the edges should be specified.
                // 2. The specified package should match the version dependency.

                let req = link.version_req();
                // A requirement of "*" filters out pre-release versions with the semver crate,
                // but cargo accepts them.
                // See https://github.com/steveklabnik/semver/issues/98.
                if !cargo_version_matches(req, to_version) {
                    return Err(Error::PackageGraphInternalError(format!(
                        "{} -> {}: version ({}) doesn't match requirement ({:?})",
                        package_id, to_id, to_version, req,
                    )));
                }

                let is_any = link.normal().is_present()
                    || link.build().is_present()
                    || link.dev().is_present();

                if !is_any {
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
    pub fn packages(&self) -> impl Iterator<Item = PackageMetadata> + ExactSizeIterator {
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
    pub fn metadata(&self, package_id: &PackageId) -> Option<PackageMetadata> {
        self.data.metadata(package_id)
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
            .map(|metadata| self.dep_links_ixs_directed(metadata.package_ix(), dir))
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

    /// Invalidates internal caches. Primarily for testing.
    #[doc(hidden)]
    pub fn invalidate_caches(&mut self) {
        mem::replace(&mut self.sccs, OnceCell::new());
        mem::replace(&mut self.feature_graph, OnceCell::new());
    }

    /// Returns the inner dependency graph.
    ///
    /// Should this be exposed publicly? Not sure.
    pub(super) fn dep_graph(&self) -> &Graph<PackageId, PackageLinkImpl, Directed, PackageIx> {
        &self.dep_graph
    }

    /// Maps an edge source, target and weight to a dependency link.
    pub(super) fn edge_to_link<'g>(
        &'g self,
        source: NodeIndex<PackageIx>,
        target: NodeIndex<PackageIx>,
        edge_ix: EdgeIndex<PackageIx>,
        edge: Option<&'g PackageLinkImpl>,
    ) -> PackageLink<'g> {
        // Note: It would be really lovely if this could just take in any EdgeRef with the right
        // parameters, but 'weight' wouldn't live long enough unfortunately.
        //
        // https://docs.rs/petgraph/0.4.13/petgraph/graph/struct.EdgeReference.html#method.weight
        // is defined separately for the same reason.
        let from = self
            .data
            .metadata_impl(&self.dep_graph[source])
            .expect("'from' should have associated metadata");
        let to = self
            .data
            .metadata_impl(&self.dep_graph[target])
            .expect("'to' should have associated metadata");
        PackageLink {
            from,
            to,
            edge_ix,
            inner: edge.unwrap_or_else(|| &self.dep_graph[edge_ix]),
        }
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
            .map(|metadata| metadata.package_ix())
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
    pub fn packages(&self) -> impl Iterator<Item = PackageMetadata> + ExactSizeIterator {
        self.packages.values().map(PackageMetadata::new)
    }

    /// Returns the metadata for the given package ID.
    pub fn metadata(&self, package_id: &PackageId) -> Option<PackageMetadata> {
        self.metadata_impl(package_id).map(PackageMetadata::new)
    }

    // ---
    // Helper methods
    // ---

    #[inline]
    pub(super) fn metadata_impl(&self, package_id: &PackageId) -> Option<&PackageMetadataImpl> {
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
    ) -> impl Iterator<Item = (&'g Path, PackageMetadata<'g>)> + ExactSizeIterator {
        let data = self.data;
        self.inner
            .members_by_path
            .iter()
            .map(move |(path, id)| (path.as_path(), data.metadata(id).expect("valid package ID")))
    }

    /// Returns an iterator over workspace names and package metadatas, sorted by names.
    pub fn members_by_name(
        &self,
    ) -> impl Iterator<Item = (&'g str, PackageMetadata<'g>)> + ExactSizeIterator {
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
    pub fn member_by_path(&self, path: impl AsRef<Path>) -> Option<PackageMetadata<'g>> {
        let id = self.inner.members_by_path.get(path.as_ref())?;
        Some(self.data.metadata(id).expect("valid package ID"))
    }

    /// Maps the given name to the corresponding workspace member.
    pub fn member_by_name(&self, name: impl AsRef<str>) -> Option<PackageMetadata<'g>> {
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

/// Information about a specific package in a `PackageGraph`.
///
/// Most of the metadata is extracted from `Cargo.toml` files. See
/// [the `Cargo.toml` reference](https://doc.rust-lang.org/cargo/reference/manifest.html) for more
/// details.
#[derive(Copy, Clone, Debug)]
pub struct PackageMetadata<'g> {
    inner: &'g PackageMetadataImpl,
}

impl<'g> PackageMetadata<'g> {
    pub(super) fn new(inner: &'g PackageMetadataImpl) -> Self {
        Self { inner }
    }

    /// Returns the unique identifier for this package.
    pub fn id(&self) -> &'g PackageId {
        &self.inner.id
    }

    /// Returns the name of this package.
    ///
    /// This is the same as the `name` field of `Cargo.toml`.
    pub fn name(&self) -> &'g str {
        &self.inner.name
    }

    /// Returns the version of this package as resolved by Cargo.
    ///
    /// This is the same as the `version` field of `Cargo.toml`.
    pub fn version(&self) -> &'g Version {
        &self.inner.version
    }

    /// Returns the authors of this package.
    ///
    /// This is the same as the `authors` field of `Cargo.toml`.
    pub fn authors(&self) -> &'g [String] {
        &self.inner.authors
    }

    /// Returns a short description for this package.
    ///
    /// This is the same as the `description` field of `Cargo.toml`.
    pub fn description(&self) -> Option<&'g str> {
        self.inner.description.as_ref().map(|x| x.as_ref())
    }

    /// Returns an SPDX 2.1 license expression for this package, if specified.
    ///
    /// This is the same as the `license` field of `Cargo.toml`. Note that `guppy` does not perform
    /// any validation on this, though `crates.io` does if a crate is uploaded there.
    pub fn license(&self) -> Option<&'g str> {
        self.inner.license.as_ref().map(|x| x.as_ref())
    }

    /// Returns the path to a license file for this package, if specified.
    ///
    /// This is the same as the `license_file` field of `Cargo.toml`. It is typically only specified
    /// for nonstandard licenses.
    pub fn license_file(&self) -> Option<&'g Path> {
        self.inner.license_file.as_ref().map(|path| path.as_ref())
    }

    /// Returns the full path to the `Cargo.toml` for this package.
    ///
    /// This is specific to the system that `cargo metadata` was run on.
    pub fn manifest_path(&self) -> &'g Path {
        &self.inner.manifest_path
    }

    /// Returns categories for this package.
    ///
    /// This is the same as the `categories` field of `Cargo.toml`. For packages on `crates.io`,
    /// returned values are guaranteed to be
    /// [valid category slugs](https://crates.io/category_slugs).
    pub fn categories(&self) -> &'g [String] {
        &self.inner.categories
    }

    /// Returns keywords for this package.
    ///
    /// This is the same as the `keywords` field of `Cargo.toml`.
    pub fn keywords(&self) -> &'g [String] {
        &self.inner.keywords
    }

    /// Returns a path to the README for this package, if specified.
    ///
    /// This is the same as the `readme` field of `Cargo.toml`. The path returned is relative to the
    /// directory the `Cargo.toml` is in (i.e. relative to the parent of `self.manifest_path()`).
    pub fn readme(&self) -> Option<&'g Path> {
        self.inner.readme.as_ref().map(|path| path.as_ref())
    }

    /// Returns the source code repository for this package, if specified.
    ///
    /// This is the same as the `repository` field of `Cargo.toml`.
    pub fn repository(&self) -> Option<&'g str> {
        self.inner.repository.as_ref().map(|x| x.as_ref())
    }

    /// Returns the Rust edition this package is written against.
    ///
    /// This is the same as the `edition` field of `Cargo.toml`. It is `"2015"` by default.
    pub fn edition(&self) -> &'g str {
        &self.inner.edition
    }

    /// Returns the freeform metadata table for this package.
    ///
    /// This is the same as the `package.metadata` section of `Cargo.toml`. This section is
    /// typically used by tools which would like to store package configuration in `Cargo.toml`.
    pub fn metadata_table(&self) -> &'g JsonValue {
        &self.inner.metadata_table
    }

    /// Returns the name of a native library this package links to, if specified.
    ///
    /// This is the same as the `links` field of `Cargo.toml`. See [The `links` Manifest
    /// Key](https://doc.rust-lang.org/cargo/reference/build-scripts.html#the-links-manifest-key) in
    /// the Cargo book for more details.
    pub fn links(&self) -> Option<&'g str> {
        self.inner.links.as_ref().map(|x| x.as_ref())
    }

    /// Returns the list of registries to which this package may be published.
    ///
    /// Returns `None` if publishing is unrestricted, and `Some(&[])` if publishing is forbidden.
    ///
    /// This is the same as the `publish` field of `Cargo.toml`.
    pub fn publish(&self) -> Option<&'g [String]> {
        self.inner.publish.as_deref()
    }

    /// Returns true if this package is in the workspace.
    pub fn in_workspace(&self) -> bool {
        self.inner.workspace_path.is_some()
    }

    /// Returns the relative path to this package in the workspace, or `None` if this package is
    /// not in the workspace.
    pub fn workspace_path(&self) -> Option<&'g Path> {
        self.inner.workspace_path.as_ref().map(|path| path.as_ref())
    }

    /// Returns all the build targets for this package.
    ///
    /// For more, see [Cargo
    /// Targets](https://doc.rust-lang.org/nightly/cargo/reference/cargo-targets.html#cargo-targets)
    /// in the Cargo reference.
    pub fn build_targets(&self) -> impl Iterator<Item = BuildTarget<'g>> {
        self.inner.build_targets.iter().map(BuildTarget::new)
    }

    /// Looks up a build target by identifier.
    pub fn build_target(&self, id: &BuildTargetId<'_>) -> Option<BuildTarget<'g>> {
        self.inner
            .build_targets
            .get_key_value(id.as_key())
            .map(BuildTarget::new)
    }

    /// Returns true if this package is a procedural macro.
    ///
    /// For more about procedural macros, see [Procedural
    /// Macros](https://doc.rust-lang.org/reference/procedural-macros.html) in the Rust reference.
    pub fn is_proc_macro(&self) -> bool {
        match self.build_target(&BuildTargetId::Library) {
            Some(build_target) => match build_target.kind() {
                BuildTargetKind::ProcMacro => true,
                _ => false,
            },
            None => false,
        }
    }

    /// Returns true if this package has a build script.
    ///
    /// Cargo only follows build dependencies if a build script is set.
    ///
    /// For more about build scripts, see [Build
    /// Scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html) in the Cargo
    /// reference.
    pub fn has_build_script(&self) -> bool {
        self.build_target(&BuildTargetId::BuildScript).is_some()
    }

    /// Returns true if this package has a named feature named `default`.
    ///
    /// For more about default features, see [The `[features]`
    /// section](https://doc.rust-lang.org/cargo/reference/manifest.html#the-features-section) in
    /// the Cargo reference.
    pub fn has_default_feature(&self) -> bool {
        self.inner.has_default_feature
    }

    /// Returns the `FeatureId` corresponding to the default feature.
    pub fn default_feature_id(&self) -> FeatureId<'g> {
        if self.inner.has_default_feature {
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
    pub fn named_features(&self) -> impl Iterator<Item = &'g str> + 'g {
        self.named_features_full()
            .map(|(_, named_feature, _)| named_feature)
    }

    // ---
    // Helper methods
    // --

    #[inline]
    pub(super) fn package_ix(&self) -> NodeIndex<PackageIx> {
        self.inner.package_ix
    }

    pub(super) fn get_feature_idx(&self, feature: &str) -> Option<usize> {
        self.inner.features.get_full(feature).map(|(n, _, _)| n)
    }

    pub(super) fn feature_idx_to_name(&self, idx: usize) -> Option<&'g str> {
        Some(
            self.inner
                .features
                .get_index(idx)
                .expect("feature idx should be valid")
                .0
                .as_ref(),
        )
    }

    #[allow(dead_code)]
    pub(super) fn all_feature_nodes(&self) -> impl Iterator<Item = FeatureNode> + 'g {
        let package_ix = self.package_ix();
        iter::once(FeatureNode::base(self.package_ix())).chain(
            (0..self.inner.features.len())
                .map(move |feature_idx| FeatureNode::new(package_ix, feature_idx)),
        )
    }

    pub(super) fn named_features_full(
        &self,
    ) -> impl Iterator<Item = (usize, &'g str, &'g [String])> + 'g {
        self.inner
            .features
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
        self.inner
            .features
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

#[derive(Clone, Debug)]
pub(crate) struct PackageMetadataImpl {
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

/// Represents a dependency from one package to another.
///
/// This struct contains information about:
/// * whether this dependency was renamed in the context of this crate.
/// * if this is a normal, dev and/or build dependency.
/// * platform-specific information about required, optional and status
#[derive(Copy, Clone, Debug)]
pub struct PackageLink<'g> {
    from: &'g PackageMetadataImpl,
    to: &'g PackageMetadataImpl,
    edge_ix: EdgeIndex<PackageIx>,
    inner: &'g PackageLinkImpl,
}

impl<'g> PackageLink<'g> {
    /// Returns the package which depends on the `to` package.
    pub fn from(&self) -> PackageMetadata<'g> {
        PackageMetadata::new(self.from)
    }

    /// Returns the package which is depended on by the `from` package.
    pub fn to(&self) -> PackageMetadata<'g> {
        PackageMetadata::new(self.to)
    }

    /// Returns the endpoints as a pair of packages `(from, to)`.
    pub fn endpoints(&self) -> (PackageMetadata<'g>, PackageMetadata<'g>) {
        (self.from(), self.to())
    }

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

    /// Returns details about this dependency from the `[dependencies]` section.
    pub fn normal(&self) -> DependencyReq<'g> {
        DependencyReq {
            inner: &self.inner.normal,
        }
    }

    /// Returns details about this dependency from the `[build-dependencies]` section.
    pub fn build(&self) -> DependencyReq<'g> {
        DependencyReq {
            inner: &self.inner.build,
        }
    }

    /// Returns details about this dependency from the `[dev-dependencies]` section.
    pub fn dev(&self) -> DependencyReq<'g> {
        DependencyReq {
            inner: &self.inner.dev,
        }
    }

    /// Returns details about this dependency from the section specified by the given dependency
    /// kind.
    pub fn req_for_kind(&self, kind: DependencyKind) -> DependencyReq<'g> {
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
        !self.normal().is_present() && !self.build().is_present()
    }

    // ---
    // Helper methods
    // ---

    /// Returns the edge index.
    #[allow(dead_code)]
    pub(super) fn edge_ix(&self) -> EdgeIndex<PackageIx> {
        self.edge_ix
    }

    /// Returns (source, target, edge) as a triple of pointers. Useful for testing.
    #[cfg(test)]
    pub(crate) fn as_inner_ptrs(
        &self,
    ) -> (
        *const PackageMetadataImpl,
        *const PackageMetadataImpl,
        *const PackageLinkImpl,
    ) {
        (self.from, self.to, self.inner)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PackageLinkImpl {
    pub(super) dep_name: String,
    pub(super) resolved_name: String,
    pub(super) version_req: VersionReq,
    pub(super) normal: DependencyReqImpl,
    pub(super) build: DependencyReqImpl,
    pub(super) dev: DependencyReqImpl,
}

/// Information about a specific kind of dependency (normal, build or dev) from a package to another
/// package.
///
/// Usually found within the context of a [`PackageLink`](struct.PackageLink.html).
#[derive(Clone, Debug)]
pub struct DependencyReq<'g> {
    pub(super) inner: &'g DependencyReqImpl,
}

impl<'g> DependencyReq<'g> {
    /// Returns true if there is at least one `Cargo.toml` entry corresponding to this requirement.
    ///
    /// For example, if this dependency is specified in the `[dev-dependencies]` section,
    /// `edge.dev().is_present()` will return true.
    pub fn is_present(&self) -> bool {
        !self.inner.enabled().is_never()
    }

    /// Returns the enabled status of this dependency.
    ///
    /// See the documentation for `EnabledStatus` for more.
    pub fn status(&self) -> EnabledStatus<'g> {
        self.inner.enabled()
    }

    /// Returns the status of default features on the platform `guppy` is running on.
    ///
    /// See the documentation for `EnabledStatus` for more.
    pub fn default_features(&self) -> EnabledStatus<'g> {
        self.inner.default_features()
    }

    /// Returns a list of all features possibly enabled by this dependency. This includes features
    /// that are only turned on if the dependency is optional, or features enabled by inactive
    /// platforms.
    pub fn features(&self) -> impl Iterator<Item = &'g str> {
        self.inner.all_features()
    }

    /// Returns the enabled status of this feature.
    ///
    /// Note that as of Rust 1.42, the default feature resolver behaves in potentially surprising
    /// ways. See the [Cargo
    /// reference](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#features) for
    /// more.
    ///
    /// See the documentation for `EnabledStatus` for more.
    pub fn feature_status(&self, feature: &str) -> EnabledStatus<'g> {
        self.inner.feature_status(feature)
    }
}

/// Whether a dependency or feature is required, optional, or disabled.
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
/// The dependency and default features are *required* on all platforms.
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
/// The result is platform-dependent. On Windows, the dependency and default features are both
/// *optional*. On non-Windows platforms, the dependency and default features are *disabled*.
///
/// ```toml
/// [dependencies]
/// once_cell = { version = "1", optional = true }
///
/// [target.'cfg(windows)'.dependencies]
/// once_cell = { version = "1", optional = false, default-features = false }
/// ```
///
/// The result is platform-dependent. On Windows, the dependency is *mandatory* and default features
/// are *optional* (i.e. enabled if the `once_cell` feature is turned on).
///
/// On Unix platforms, the dependency and default features are both *optional*.
#[derive(Copy, Clone, Debug)]
pub struct EnabledStatus<'g> {
    required: PlatformStatus<'g>,
    optional: PlatformStatus<'g>,
}

impl<'g> EnabledStatus<'g> {
    pub(super) fn new(required: &'g PlatformStatusImpl, optional: &'g PlatformStatusImpl) -> Self {
        Self {
            required: PlatformStatus::new(required),
            optional: PlatformStatus::new(optional),
        }
    }

    /// Returns true if this dependency is required on all platforms.
    pub fn is_always_required(&self) -> bool {
        self.required.is_always()
    }

    /// Returns true if this dependency is never enabled on any platform.
    pub fn is_never(&self) -> bool {
        self.required.is_never() && self.optional.is_never()
    }

    /// Evaluates whether this dependency is required on the given platform.
    ///
    /// Returns `Unknown` if the result was unknown, which may happen if the platform's target
    /// features are unknown.
    pub fn required_on(&self, platform: &Platform<'_>) -> EnabledTernary {
        self.required.enabled_on(platform)
    }

    /// Evaluates whether this dependency is enabled (required or optional) on the given platform.
    ///
    /// Returns `Unknown` if the result was unknown, which may happen if the platform's target
    /// features are unknown.
    pub fn enabled_on(&self, platform: &Platform<'_>) -> EnabledTernary {
        let required = self.required.enabled_on(platform);
        let optional = self.optional.enabled_on(platform);

        required.or(optional)
    }

    /// Returns the `PlatformStatus` corresponding to whether this dependency is required.
    pub fn required_status(&self) -> PlatformStatus<'g> {
        self.required
    }

    /// Returns the `PlatformStatus` corresponding to whether this dependency is optional.
    pub fn optional_status(&self) -> PlatformStatus<'g> {
        self.optional
    }
}

/// The status of a dependency or feature, which is possibly platform-dependent.
///
/// This is a sub-status of `EnabledStatus`.
#[derive(Copy, Clone, Debug)]
pub enum PlatformStatus<'g> {
    /// This dependency or feature is never enabled on any platforms.
    Never,
    /// This dependency or feature is always enabled on all platforms.
    Always,
    /// The status is platform-dependent.
    PlatformDependent {
        /// An evaluator to run queries against.
        eval: PlatformEval<'g>,
    },
}

impl<'g> PlatformStatus<'g> {
    fn new(specs: &'g PlatformStatusImpl) -> Self {
        match specs {
            PlatformStatusImpl::Always => PlatformStatus::Always,
            PlatformStatusImpl::Specs(specs) => {
                if specs.is_empty() {
                    PlatformStatus::Never
                } else {
                    PlatformStatus::PlatformDependent {
                        eval: PlatformEval { specs },
                    }
                }
            }
        }
    }

    /// Returns true if this dependency is always enabled on all platforms.
    pub fn is_always(&self) -> bool {
        match self {
            PlatformStatus::Always => true,
            PlatformStatus::PlatformDependent { .. } | PlatformStatus::Never => false,
        }
    }

    /// Returns true if this dependency is never enabled on any platform.
    pub fn is_never(&self) -> bool {
        match self {
            PlatformStatus::Never => true,
            PlatformStatus::PlatformDependent { .. } | PlatformStatus::Always => false,
        }
    }

    /// Evaluates whether this dependency is enabled on the given platform.
    pub fn enabled_on(&self, platform: &Platform<'g>) -> EnabledTernary {
        match self {
            PlatformStatus::Never => EnabledTernary::Disabled,
            PlatformStatus::Always => EnabledTernary::Enabled,
            PlatformStatus::PlatformDependent { eval } => eval.eval(platform),
        }
    }
}

/// Whether a dependency or feature is enabled on a specific platform.
///
/// This is a ternary or [three-valued logic](https://en.wikipedia.org/wiki/Three-valued_logic)
/// because the result may be unknown in some situations.
///
/// Returned by the methods on `EnabledStatus`, `PlatformStatus`, and `PlatformEval`.
#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EnabledTernary {
    /// The dependency is disabled on this platform.
    Disabled,
    /// The status of this dependency is unknown on this platform.
    ///
    /// This may happen if evaluation involves unknown target features. Notably, this will not be
    /// returned for `Platform::current()`, since the target features for the current platform are
    /// known.
    Unknown,
    /// The dependency is enabled on this platform.
    Enabled,
}

impl EnabledTernary {
    fn new(x: Option<bool>) -> Self {
        match x {
            Some(false) => EnabledTernary::Disabled,
            None => EnabledTernary::Unknown,
            Some(true) => EnabledTernary::Enabled,
        }
    }

    /// Returns true if the status is known (either enabled or disabled).
    pub fn is_known(self) -> bool {
        match self {
            EnabledTernary::Disabled | EnabledTernary::Enabled => true,
            EnabledTernary::Unknown => false,
        }
    }

    /// OR operation in Kleene K3 logic.
    fn or(self, other: Self) -> Self {
        use EnabledTernary::*;

        match (self, other) {
            (Disabled, Disabled) => Disabled,
            (Enabled, _) | (_, Enabled) => Enabled,
            _ => Unknown,
        }
    }
}

/// An evaluator for platform-specific dependencies.
///
/// This represents a collection of platform specifications, of the sort `cfg(unix)`.
///
/// For more, see [Platform specific
/// dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#platform-specific-dependencies)
/// in the Cargo reference.
#[derive(Copy, Clone, Debug)]
pub struct PlatformEval<'g> {
    specs: &'g [TargetSpec],
}

impl<'g> PlatformEval<'g> {
    /// Runs this evaluator against the given platform.
    pub fn eval(&self, platform: &Platform<'_>) -> EnabledTernary {
        let mut res = EnabledTernary::Disabled;
        for spec in self.specs.iter() {
            let matches = spec.eval(platform);
            // Short-circuit evaluation if possible.
            if matches == Some(true) {
                return EnabledTernary::Enabled;
            }
            res = res.or(EnabledTernary::new(matches));
        }
        res
    }
}

/// Information about dependency requirements.
#[derive(Clone, Debug, Default)]
pub(super) struct DependencyReqImpl {
    pub(super) required: DepRequiredOrOptional,
    pub(super) optional: DepRequiredOrOptional,
}

impl DependencyReqImpl {
    fn all_features(&self) -> impl Iterator<Item = &str> {
        self.required
            .all_features()
            .chain(self.optional.all_features())
    }

    pub(super) fn enabled(&self) -> EnabledStatus {
        self.make_status(|req_impl| &req_impl.build_if)
    }

    pub(super) fn default_features(&self) -> EnabledStatus {
        self.make_status(|req_impl| &req_impl.default_features_if)
    }

    pub(super) fn feature_status(&self, feature: &str) -> EnabledStatus {
        // This PlatformStatusImpl in static memory is so that the lifetimes work out.
        static DEFAULT_STATUS: PlatformStatusImpl = PlatformStatusImpl::Specs(Vec::new());

        self.make_status(|req_impl| {
            req_impl
                .feature_targets
                .get(feature)
                .unwrap_or(&DEFAULT_STATUS)
        })
    }

    fn make_status(
        &self,
        pred_fn: impl Fn(&DepRequiredOrOptional) -> &PlatformStatusImpl,
    ) -> EnabledStatus {
        EnabledStatus::new(pred_fn(&self.required), pred_fn(&self.optional))
    }
}

/// Information about dependency requirements, scoped to either the dependency being required or
/// optional.
#[derive(Clone, Debug, Default)]
pub(super) struct DepRequiredOrOptional {
    pub(super) build_if: PlatformStatusImpl,
    pub(super) default_features_if: PlatformStatusImpl,
    pub(super) feature_targets: BTreeMap<String, PlatformStatusImpl>,
}

impl DepRequiredOrOptional {
    pub(super) fn all_features(&self) -> impl Iterator<Item = &str> {
        self.feature_targets.keys().map(|s| s.as_str())
    }
}

#[derive(Clone, Debug)]
pub(crate) enum PlatformStatusImpl {
    Always,
    // Empty vector means never.
    Specs(Vec<TargetSpec>),
}

impl PlatformStatusImpl {
    /// Returns true if this is an empty predicate (i.e. will never match).
    pub(super) fn is_never(&self) -> bool {
        match self {
            PlatformStatusImpl::Always => false,
            PlatformStatusImpl::Specs(specs) => specs.is_empty(),
        }
    }
}

impl Default for PlatformStatusImpl {
    fn default() -> Self {
        // Empty vector means never.
        PlatformStatusImpl::Specs(vec![])
    }
}
