use crate::errors::Error;
use cargo_metadata::{Dependency, Metadata, MetadataCommand, NodeDep, Package, PackageId, Resolve};
use petgraph::prelude::*;
use semver::Version;
use std::collections::{HashMap, HashSet};

pub struct PackageGraph {
    packages: HashMap<PackageId, PackageMetadata>,
    dep_graph: Graph<PackageId, ()>,
}

#[derive(Clone, Debug)]
pub struct PackageMetadata {
    // Fields extracted from the package.
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

        let mut build_state = GraphBuildState::new(resolve, metadata.workspace_members);

        let packages: HashMap<_, _> = metadata
            .packages
            .into_iter()
            .map(|package| build_state.process_package(package))
            .collect::<Result<_, _>>()?;

        let dep_graph = build_state.finish();

        Ok(Self {
            packages,
            dep_graph,
        })
    }
}

/// Helper struct for building up dependency graph.
struct GraphBuildState {
    dep_graph: Graph<PackageId, ()>,
    id_to_node: HashMap<PackageId, NodeIndex<u32>>,
    resolve_data: HashMap<PackageId, (Vec<NodeDep>, Vec<String>)>,
    workspace_members: HashSet<PackageId>,
}

impl GraphBuildState {
    fn new(resolve: Resolve, workspace_members: Vec<PackageId>) -> Self {
        let mut dep_graph = Graph::new();
        let id_to_node: HashMap<_, _> = resolve
            .nodes
            .iter()
            .map(|node| (node.id.clone(), dep_graph.add_node(node.id.clone())))
            .collect();

        let resolve_data: HashMap<_, _> = resolve
            .nodes
            .into_iter()
            .map(|node| (node.id, (node.deps, node.features)))
            .collect();

        let workspace_members = workspace_members.into_iter().collect::<HashSet<_>>();

        Self {
            dep_graph,
            id_to_node,
            resolve_data,
            workspace_members,
        }
    }

    fn process_package(&mut self, package: Package) -> Result<(PackageId, PackageMetadata), Error> {
        let node_idx = self.node_idx(&package.id)?;
        let in_workspace = self.workspace_members.contains(&package.id);
        let (resolved_deps, resolved_features) =
            self.resolve_data.remove(&package.id).ok_or_else(|| {
                Error::DepGraphError(format!(
                    "no resolved dependency data found for package '{}'",
                    package.id
                ))
            })?;

        // TODO: track features and normal/build/dev
        for dep in &resolved_deps {
            let dep_idx = self.node_idx(&dep.pkg)?;
            self.dep_graph.add_edge(node_idx, dep_idx, ());
        }

        Ok((
            package.id,
            PackageMetadata {
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

    fn node_idx(&self, id: &PackageId) -> Result<NodeIndex<u32>, Error> {
        self.id_to_node
            .get(&id)
            .copied()
            .ok_or_else(|| Error::DepGraphError(format!("no node data found for package '{}'", id)))
    }

    fn finish(self) -> Graph<PackageId, ()> {
        self.dep_graph
    }
}
