// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Implementations for options shared by commands.

use clap::arg_enum;
use guppy::graph::{DependencyLink, EnabledStatus, PackageGraph};
use guppy::{Platform, TargetFeatures};
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

        let mut omitted_package_ids = HashSet::new();
        for metadata in pkg_graph.packages() {
            if omitted_set.contains(metadata.name()) {
                omitted_package_ids.insert(metadata.id().clone());
            }
        }

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
