use crate::errors::Error;
use auto_enums::auto_enum;
use cargo_metadata::{
    Dependency, DependencyKind, Metadata, MetadataCommand, NodeDep, Package, PackageId, Resolve,
};
use lazy_static::lazy_static;
use petgraph::prelude::*;
use petgraph::visit::Visitable;
use semver::{Version, VersionReq};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

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

    pub fn new(metadata: Metadata) -> Result<Self, Error> {
        let resolve = metadata.resolve.ok_or_else(|| {
            Error::DepGraphError(
                "no 'resolve' entries found: ensure you don't have no_deps set".into(),
            )
        })?;

        let workspace_members: HashSet<_> = metadata.workspace_members.into_iter().collect();

        let mut build_state = GraphBuildState::new(&metadata.packages, resolve, &workspace_members);

        let packages: HashMap<_, _> = metadata
            .packages
            .into_iter()
            .map(|package| build_state.process_package(package))
            .collect::<Result<_, _>>()?;

        let dep_graph = build_state.finish();

        let workspace = Workspace::new(metadata.workspace_root, &packages, workspace_members)?;

        Ok(Self {
            packages,
            dep_graph,
            workspace,
        })
    }

    /// Verifies internal invariants on this graph.
    pub fn verify(&self) -> Result<(), Error> {
        lazy_static! {
            static ref MAJOR_WILDCARD: VersionReq = VersionReq::parse("*").unwrap();
        }

        for (package_id, metadata) in self.packages() {
            for dep in self.deps(package_id) {
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

    pub fn deps<'a>(&'a self, package_id: &PackageId) -> impl Iterator<Item = PackageDep<'a>> + 'a {
        self.deps_directed(package_id, Outgoing)
    }

    // XXX fix this up: this should be a PackageDep but Bfs only returns nodes. Probably need to
    // implement a custom BFS ourselves with edge indexes.
    // Also this would ideally be an iterator but that seems harder.
    pub fn transitive_deps<'a, 'b>(
        &'a self,
        package_ids: impl IntoIterator<Item = &'b PackageId>,
    ) -> Result<Vec<&'a PackageId>, Error> {
        let node_idxs = package_ids
            .into_iter()
            .map(|package_id| {
                let package_metadata = self
                    .metadata(package_id)
                    .ok_or_else(|| Error::DepGraphUnknownPackageId(package_id.clone()))?;
                Ok(package_metadata.node_idx)
            })
            .collect::<Result<VecDeque<_>, Error>>()?;

        let mut bfs = Bfs {
            stack: node_idxs,
            discovered: self.dep_graph.visit_map(),
        };

        let mut transitive_deps = vec![];
        while let Some(node_idx) = bfs.next(&self.dep_graph) {
            let dep_id = self
                .dep_graph
                .node_weight(node_idx)
                .expect("bfs should return a valid node");
            transitive_deps.push(dep_id);
        }
        Ok(transitive_deps)
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
pub struct Workspace {
    root: PathBuf,
    // This is a BTreeMap to allow presenting data in sorted order.
    members_by_path: BTreeMap<PathBuf, PackageId>,
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
                    Error::DepGraphError(format!("workspace member '{}' not found", id))
                })?;
                let manifest_path = &package_metadata.manifest_path;
                let workspace_path = manifest_path.strip_prefix(&workspace_root).map_err(|_| {
                    Error::DepGraphError(format!(
                        "workspace member '{}' at path {:?} not in workspace (root: {:?})",
                        id, manifest_path, workspace_root
                    ))
                })?;
                let workspace_path = workspace_path.parent().ok_or_else(|| {
                    Error::DepGraphError(format!(
                        "workspace member '{}' has invalid manifest path {:?}",
                        id, manifest_path
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
    workspace_members: &'a HashSet<PackageId>,
}

impl<'a> GraphBuildState<'a> {
    fn new<'b>(
        packages: impl IntoIterator<Item = &'b Package>,
        resolve: Resolve,
        workspace_members: &'a HashSet<PackageId>,
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
            self.dep_graph.update_edge(node_idx, dep_idx, edge);
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
                manifest_path: package.manifest_path,

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

struct DependencyResolver<'a> {
    from_id: &'a PackageId,

    /// The package data, inherited from the graph build state.
    package_data: &'a HashMap<PackageId, (NodeIndex<u32>, String, Version)>,

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
        package_data: &'a HashMap<PackageId, (NodeIndex<u32>, String, Version)>,
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
            Error::DepGraphError(format!(
                "{}: no package data found for dependency '{}'",
                self.from_id, package_id
            ))
        })?;

        // Lookup the name in the original map.
        let (name, deps) = self
            .original_map
            .get(package_name.as_str())
            .ok_or_else(|| {
                Error::DepGraphError(format!(
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
                Some(old) => match (&old.target, &metadata.target) {
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
                        return Err(Error::DepGraphError(format!(
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

fn kind_str(kind: DependencyKind) -> &'static str {
    match kind {
        DependencyKind::Normal => "normal",
        DependencyKind::Build => "build",
        DependencyKind::Development => "dev",
        _ => "unknown",
    }
}
