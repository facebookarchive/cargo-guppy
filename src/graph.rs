// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

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

        let mut build_state =
            GraphBuildState::new(&metadata.packages, resolve, metadata.workspace_members);

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
    // The value is (node_idx, name, version)
    package_data: HashMap<PackageId, (NodeIndex<u32>, String, Version)>,
    resolve_data: HashMap<PackageId, (Vec<NodeDep>, Vec<String>)>,
    workspace_members: HashSet<PackageId>,
}

impl GraphBuildState {
    fn new<'a>(
        packages: impl IntoIterator<Item = &'a Package>,
        resolve: Resolve,
        workspace_members: Vec<PackageId>,
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

        let workspace_members = workspace_members.into_iter().collect::<HashSet<_>>();

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

        // TODO: track features and normal/build/dev
        for dep in &resolved_deps {
            let (dep_idx, _, _) = self.package_data(&dep.pkg)?;
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

    fn package_data<'a>(
        &'a self,
        id: &PackageId,
    ) -> Result<(NodeIndex<u32>, &'a str, &'a Version), Error> {
        let (node_idx, name, version) = self.package_data.get(&id).ok_or_else(|| {
            Error::DepGraphError(format!("no package data found for package '{}'", id))
        })?;
        Ok((*node_idx, name, version))
    }

    fn finish(self) -> Graph<PackageId, ()> {
        self.dep_graph
    }
}
