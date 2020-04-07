// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Implementations for options shared by commands.

use anyhow::{anyhow, ensure};
use clap::arg_enum;
use guppy::graph::{
    DependencyDirection, DependencyLink, EnabledStatus, PackageGraph, PackageSelect,
};
use guppy::{PackageId, Platform, TargetFeatures};
use std::collections::HashSet;
use structopt::StructOpt;

arg_enum! {
    #[derive(Debug)]
    pub enum Kind {
        All,
        Workspace,
        DirectThirdParty,
        ThirdParty,
    }
}

#[derive(Debug, StructOpt)]
pub struct SelectOptions {
    /// Select reverse transitive dependencies (default: forward)
    #[structopt(long = "select-reverse", parse(from_flag = parse_direction))]
    direction: DependencyDirection,

    #[structopt(rename_all = "screaming_snake_case")]
    /// The root packages to start the selection from
    roots: Vec<String>,
}

impl SelectOptions {
    /// Constructs a `PackageSelect` based on these options.
    pub fn apply<'g>(
        &self,
        pkg_graph: &'g PackageGraph,
    ) -> Result<PackageSelect<'g>, anyhow::Error> {
        if !self.roots.is_empty() {
            // NOTE: The root set packages are specified by name. The tool currently
            // does not handle multiple version of the same package as the current use
            // cases are passing workspace members as the root set, which won't be
            // duplicated.
            let root_set = self.roots.iter().map(|s| s.as_str()).collect();
            Ok(pkg_graph.select_directed(names_to_ids(&pkg_graph, &root_set), self.direction)?)
        } else {
            ensure!(
                self.direction == DependencyDirection::Forward,
                anyhow!("--select-reverse requires roots to be specified")
            );
            Ok(pkg_graph.select_workspace())
        }
    }
}

#[derive(Debug, StructOpt)]
pub struct FilterOptions {
    #[structopt(long, short, possible_values = &Kind::variants(), case_insensitive = true, default_value = "all")]
    /// Kind of crates to select
    pub kind: Kind,

    #[structopt(long, rename_all = "kebab-case")]
    /// Include dev dependencies
    pub include_dev: bool,

    #[structopt(long, rename_all = "kebab-case")]
    /// Include build dependencies
    pub include_build: bool,

    #[structopt(long)]
    /// Target to select for, default is to match all targets
    pub target: Option<String>,

    #[structopt(
        long,
        rename_all = "kebab-case",
        name = "package",
        number_of_values = 1
    )]
    /// Omit edges that point into a given package; useful for seeing how
    /// removing a dependency affects the graph
    pub omit_edges_into: Vec<String>,
}

impl FilterOptions {
    /// Construct a package resolver based on the filter options.
    pub fn make_resolver<'g>(
        &'g self,
        pkg_graph: &'g PackageGraph,
    ) -> impl Fn(DependencyLink<'g>) -> bool + 'g {
        let omitted_set: HashSet<&str> = self.omit_edges_into.iter().map(|s| s.as_str()).collect();
        let omitted_package_ids: HashSet<_> = names_to_ids(pkg_graph, &omitted_set).collect();

        let platform = if let Some(ref target) = self.target {
            // The features are unknown.
            Some(Platform::new(target, TargetFeatures::Unknown).unwrap())
        } else {
            None
        };

        move |DependencyLink { from, to, edge }| {
            // filter by the kind of dependency (--kind)
            // NOTE: We always retain all workspace deps in the graph, otherwise
            // we'll get a disconnected graph.
            let include_kind = match self.kind {
                Kind::All | Kind::ThirdParty => true,
                Kind::DirectThirdParty => from.in_workspace(),
                Kind::Workspace => from.in_workspace() && to.in_workspace(),
            };

            // filter out irrelevant dependencies for a specific target (--target)
            let include_target = if let Some(platform) = &platform {
                edge.normal()
                    .map(|meta| {
                        // Include this dependency if it's optional or mandatory or if the status is
                        // unknown.
                        meta.enabled_on(platform) != EnabledStatus::Never
                    })
                    .unwrap_or(true)
            } else {
                true
            };

            // filter normal, dev, and build dependencies (--include-build, --include-dev)
            let include_type = edge.normal().is_some()
                || self.include_dev && edge.dev().is_some()
                || self.include_build && edge.build().is_some();

            // filter out provided edge targets (--omit-edges-into)
            let include_edge = !omitted_package_ids.contains(to.id());

            include_kind && include_target && include_type && include_edge
        }
    }
}

pub(crate) fn parse_direction(reverse: bool) -> DependencyDirection {
    if reverse {
        DependencyDirection::Reverse
    } else {
        DependencyDirection::Forward
    }
}

pub(crate) fn names_to_ids<'g: 'a, 'a>(
    pkg_graph: &'g PackageGraph,
    names: &'a HashSet<&str>,
) -> impl Iterator<Item = &'g PackageId> + 'a {
    pkg_graph.packages().filter_map(move |metadata| {
        if names.contains(metadata.name()) {
            Some(metadata.id())
        } else {
            None
        }
    })
}
