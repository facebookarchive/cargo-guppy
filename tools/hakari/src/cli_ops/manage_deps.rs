// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Add and remove dependencies.

use crate::{
    cli_ops::{WorkspaceOp, WorkspaceOps},
    HakariBuilder,
};
use guppy::graph::{DependencyDirection, PackageSet};

impl<'g> HakariBuilder<'g> {
    /// Returns the set of operations that need to be performed to add the workspace-hack
    /// dependency to the given set of workspace crates.
    ///
    /// Also includes remove operations for the workspace-hack dependency from excluded crates.
    ///
    /// Returns `None` if the hakari package wasn't specified at construction time.
    ///
    /// Requires the `cli-support` feature to be enabled.
    pub fn manage_dep_ops(&self, workspace_set: &PackageSet<'g>) -> Option<WorkspaceOps<'g, '_>> {
        let graph = self.graph();
        let hakari_package = self.hakari_package()?;

        let (add_to, remove_from) =
            workspace_set.filter_partition(DependencyDirection::Reverse, |package| {
                let currently_included = graph
                    .directly_depends_on(package.id(), hakari_package.id())
                    .expect("valid package ID");
                let should_be_included = !self.is_excluded(package.id()).expect("valid package ID");
                match (currently_included, should_be_included) {
                    (false, true) => Some(true),
                    (true, false) => Some(false),
                    _ => None,
                }
            });

        let mut ops = Vec::with_capacity(2);
        if !add_to.is_empty() {
            ops.push(WorkspaceOp::AddDependency {
                name: hakari_package.name(),
                crate_path: hakari_package
                    .source()
                    .workspace_path()
                    .expect("hakari package is in workspace"),
                add_to,
            });
        }
        if !remove_from.is_empty() {
            ops.push(WorkspaceOp::RemoveDependency {
                name: hakari_package.name(),
                remove_from,
            });
        }
        Some(WorkspaceOps::new(graph, ops))
    }

    /// Returns the set of operations that need to be performed to add the workspace-hack
    /// dependency to the given set of workspace crates.
    ///
    /// Returns `None` if the hakari package wasn't specified at construction time.
    ///
    /// Requires the `cli-support` feature to be enabled.
    pub fn add_dep_ops(
        &self,
        workspace_set: &PackageSet<'g>,
        force: bool,
    ) -> Option<WorkspaceOps<'g, '_>> {
        let graph = self.graph();
        let hakari_package = self.hakari_package()?;

        let add_to = if force {
            workspace_set.clone()
        } else {
            workspace_set.filter(DependencyDirection::Reverse, |package| {
                !graph
                    .directly_depends_on(package.id(), hakari_package.id())
                    .expect("valid package ID")
            })
        };

        let op = if !add_to.is_empty() {
            Some(WorkspaceOp::AddDependency {
                name: hakari_package.name(),
                crate_path: hakari_package
                    .source()
                    .workspace_path()
                    .expect("hakari package is in workspace"),
                add_to,
            })
        } else {
            None
        };
        Some(WorkspaceOps::new(graph, op))
    }

    /// Returns the set of operations that need to be performed to remove the workspace-hack
    /// dependency from the given set of workspace crates.
    ///
    /// Returns `None` if the hakari package wasn't specified at construction time.
    ///
    /// Requires the `cli-support` feature to be enabled.
    pub fn remove_dep_ops(
        &self,
        workspace_set: &PackageSet<'g>,
        force: bool,
    ) -> Option<WorkspaceOps<'g, '_>> {
        let graph = self.graph();
        let hakari_package = self.hakari_package()?;

        let remove_from = if force {
            workspace_set.clone()
        } else {
            workspace_set.filter(DependencyDirection::Reverse, |package| {
                graph
                    .directly_depends_on(package.id(), hakari_package.id())
                    .expect("valid package ID")
            })
        };

        let op = if !remove_from.is_empty() {
            Some(WorkspaceOp::RemoveDependency {
                name: hakari_package.name(),
                remove_from,
            })
        } else {
            None
        };
        Some(WorkspaceOps::new(graph, op))
    }
}
