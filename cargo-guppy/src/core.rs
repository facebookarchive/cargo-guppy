// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Implementations for options shared by commands.

use anyhow::{anyhow, ensure};
use clap::arg_enum;
use guppy::graph::feature::{CrossEdge, CrossLink, FeatureQuery, FeatureResolver};
use guppy::graph::{
    DependencyDirection, DependencyReq, EnabledStatus, EnabledTernary, PackageEdge, PackageGraph,
    PackageLink, PackageQuery, PackageResolver, PlatformStatus,
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
pub struct QueryOptions {
    /// Query reverse transitive dependencies (default: forward)
    #[structopt(long = "query-reverse", parse(from_flag = parse_direction))]
    direction: DependencyDirection,

    #[structopt(rename_all = "screaming_snake_case")]
    /// The root packages to start the query from
    roots: Vec<String>,
}

impl QueryOptions {
    /// Constructs a `PackageQuery` based on these options.
    pub fn apply<'g>(
        &self,
        pkg_graph: &'g PackageGraph,
    ) -> Result<PackageQuery<'g>, anyhow::Error> {
        if !self.roots.is_empty() {
            // NOTE: The root set packages are specified by name. The tool currently
            // does not handle multiple version of the same package as the current use
            // cases are passing workspace members as the root set, which won't be
            // duplicated.
            let root_set = self.roots.iter().map(|s| s.as_str()).collect();
            Ok(pkg_graph.query_directed(names_to_ids(&pkg_graph, &root_set), self.direction)?)
        } else {
            ensure!(
                self.direction == DependencyDirection::Forward,
                anyhow!("--query-reverse requires roots to be specified")
            );
            Ok(pkg_graph.query_workspace())
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
    /// Target to filter, default is to match all targets
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
    ) -> impl Fn(&PackageQuery<'g>, PackageLink<'g>) -> bool + 'g {
        let omitted_set: HashSet<&str> = self.omit_edges_into.iter().map(|s| s.as_str()).collect();
        let omitted_package_ids: HashSet<_> = names_to_ids(pkg_graph, &omitted_set).collect();

        let platform = if let Some(ref target) = self.target {
            // The features are unknown.
            Some(Platform::new(target, TargetFeatures::Unknown).unwrap())
        } else {
            None
        };

        move |_, PackageLink { from, to, edge }| {
            // filter by the kind of dependency (--kind)
            // NOTE: We always retain all workspace deps in the graph, otherwise
            // we'll get a disconnected graph.
            let include_kind = match self.kind {
                Kind::All | Kind::ThirdParty => true,
                Kind::DirectThirdParty => from.in_workspace(),
                Kind::Workspace => from.in_workspace() && to.in_workspace(),
            };

            let include_type = if let Some(platform) = &platform {
                // filter out irrelevant dependencies for a specific target (--target)
                self.eval(edge, |req| {
                    req.status().enabled_on(platform) != EnabledTernary::Disabled
                })
            } else {
                // keep dependencies that are potentially enabled on any platform
                self.eval(edge, |req| req.is_present())
            };

            // --initial-edges=normal,build,dev --dep-edges=normal,build

            // filter out provided edge targets (--omit-edges-into)
            let include_edge = !omitted_package_ids.contains(to.id());

            include_kind && include_type && include_edge
        }
    }

    /// Select normal, dev, or build dependencies as requested (--include-build, --include-dev), and
    /// apply `pred_fn` to whatever's selected.
    fn eval(
        &self,
        edge: PackageEdge<'_>,
        mut pred_fn: impl FnMut(DependencyReq<'_>) -> bool,
    ) -> bool {
        pred_fn(edge.normal())
            || self.include_dev && pred_fn(edge.dev())
            || self.include_build && pred_fn(edge.build())
    }

    /// Construct a feature resolver based on the filter options.
    pub fn make_feature_resolver<'g>(
        &'g self,
        pkg_graph: &'g PackageGraph,
    ) -> impl Fn(&FeatureQuery<'g>, CrossLink<'g>) -> bool + 'g {
        let omitted_set: HashSet<&str> = self.omit_edges_into.iter().map(|s| s.as_str()).collect();
        let omitted_package_ids: HashSet<_> = names_to_ids(pkg_graph, &omitted_set).collect();

        let platform = if let Some(ref target) = self.target {
            // The features are unknown.
            Some(Platform::new(target, TargetFeatures::Unknown).unwrap())
        } else {
            None
        };

        move |_, CrossLink { from, to, edge }| {
            let from_package = from.package();
            let to_package = to.package();

            // filter by the kind of dependency (--kind)
            // NOTE: We always retain all workspace deps in the graph, otherwise
            // we'll get a disconnected graph.
            let include_kind = match self.kind {
                Kind::All | Kind::ThirdParty => true,
                Kind::DirectThirdParty => from_package.in_workspace(),
                Kind::Workspace => from_package.in_workspace() && to_package.in_workspace(),
            };

            let include_type = if let Some(platform) = &platform {
                // filter out irrelevant dependencies for a specific target (--target)
                self.eval_feature(edge, |status| {
                    status.enabled_on(platform) != EnabledTernary::Disabled
                })
            } else {
                self.eval_feature(edge, |status| !status.is_never())
            };

            // filter out provided edge targets (--omit-edges-into)
            let include_edge = !omitted_package_ids.contains(to.package_id());

            include_kind && include_type && include_edge
        }
    }

    fn eval_feature(
        &self,
        edge: CrossEdge<'_>,
        mut pred_fn: impl FnMut(PlatformStatus<'_>) -> bool,
    ) -> bool {
        pred_fn(edge.normal())
            || self.include_dev && pred_fn(edge.dev())
            || self.include_build && pred_fn(edge.build())
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

pub(crate) fn triple_to_platform(
    triple: Option<&String>,
) -> Result<Option<Platform>, anyhow::Error> {
    match triple {
        Some(triple) => match Platform::new(triple, TargetFeatures::Unknown) {
            Some(platform) => Ok(Some(platform)),
            None => Err(anyhow!("unrecognized triple '{}'", triple)),
        },
        None => Ok(None),
    }
}
