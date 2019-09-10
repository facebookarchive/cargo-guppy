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
            .map(|package| build_state.add_package(package))
            .collect::<Result<_, _>>()?;

        let mut dep_graph = build_state.finish();

        for (id, data) in &packages {
            for dep in &data.resolved_deps {
                let dep_id = &dep.pkg;
                let dep_data = packages.get(dep_id).ok_or_else(|| {
                    Error::DepGraphError(format!(
                        "for package '{}', no package data found for dependency '{}'",
                        id, dep_id
                    ))
                })?;

                dep_graph.add_edge(data.node_idx, dep_data.node_idx, ());
            }
        }

        Ok(Self {
            packages,
            dep_graph,
        })
    }
}

// Helper struct for building up graph nodes (not edges, those are done afterwards)
struct GraphBuildState {
    dep_graph: Graph<PackageId, ()>,
    resolve_data: HashMap<PackageId, (Vec<NodeDep>, Vec<String>)>,
    workspace_members: HashSet<PackageId>,
}

impl GraphBuildState {
    fn new(resolve: Resolve, workspace_members: Vec<PackageId>) -> Self {
        let dep_graph = Graph::new();

        let resolve_data: HashMap<_, _> = resolve
            .nodes
            .into_iter()
            .map(|node| (node.id, (node.deps, node.features)))
            .collect();

        let workspace_members = workspace_members.into_iter().collect::<HashSet<_>>();

        Self {
            dep_graph,
            workspace_members,
            resolve_data,
        }
    }

    fn add_package(&mut self, package: Package) -> Result<(PackageId, PackageMetadata), Error> {
        let node_idx = self.dep_graph.add_node(package.id.clone());
        let in_workspace = self.workspace_members.contains(&package.id);
        let (resolved_deps, resolved_features) =
            self.resolve_data.remove(&package.id).ok_or_else(|| {
                Error::DepGraphError(format!(
                    "no resolve data found for package '{}'",
                    package.id
                ))
            })?;
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

    fn finish(self) -> Graph<PackageId, ()> {
        self.dep_graph
    }
}
