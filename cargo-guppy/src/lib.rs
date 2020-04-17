// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

mod core;
mod diff;

pub use crate::core::*;

use anyhow;
use guppy::graph::feature::{all_filter, default_filter, CargoOptions};
use guppy::graph::DependencyDirection;
use guppy::{
    graph::{DotWrite, PackageDotVisitor, PackageGraph, PackageLink, PackageMetadata},
    MetadataCommand, PackageId, Platform, TargetFeatures,
};
use itertools;
use std::cmp;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::io::Write;
use std::iter;
use structopt::StructOpt;

pub fn cmd_diff(json: bool, old: &str, new: &str) -> Result<(), anyhow::Error> {
    let old_json = fs::read_to_string(old)?;
    let new_json = fs::read_to_string(new)?;

    let old_graph = PackageGraph::from_json(&old_json)?;
    let new_graph = PackageGraph::from_json(&new_json)?;

    let old_packages: Vec<_> = old_graph.packages().collect();
    let new_packages: Vec<_> = new_graph.packages().collect();

    let diff = diff::DiffOptions::default().diff(&old_packages, &new_packages);

    if json {
        println!("{}", serde_json::to_string_pretty(&diff).unwrap());
    } else {
        print!("{}", diff);
    }

    Ok(())
}

pub fn cmd_dups(filter_opts: &FilterOptions) -> Result<(), anyhow::Error> {
    let mut command = MetadataCommand::new();
    let pkg_graph = PackageGraph::from_command(&mut command)?;

    let resolver = filter_opts.make_resolver(&pkg_graph);
    let selection = pkg_graph.query_workspace();

    let mut dupe_map: HashMap<_, Vec<_>> = HashMap::new();
    for package in selection
        .resolve_with_fn(resolver)
        .into_metadatas(DependencyDirection::Forward)
    {
        dupe_map.entry(package.name()).or_default().push(package);
    }

    for (name, dupes) in dupe_map {
        if dupes.len() <= 1 {
            continue;
        }

        let output = itertools::join(dupes.iter().map(|p| p.version()), ", ");

        println!("{} ({})", name, output);
    }

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct ResolveCargoOptions {
    #[structopt(long = "package", short = "p")]
    packages: Vec<String>,

    #[structopt(long = "include-dev")]
    /// Include dev-dependencies of initial packages (default: false)
    include_dev: bool,

    #[structopt(long = "target-platform")]
    /// Evaluate against target platform triple (default: any)
    target_platform: Option<String>,

    #[structopt(long = "host-platform")]
    /// Evaluate against host platform triple (default: target platform)
    host_platform: Option<String>,
}

pub fn cmd_resolve_cargo(opts: &ResolveCargoOptions) -> Result<(), anyhow::Error> {
    let target_platform = triple_to_platform(opts.target_platform.as_ref())?;
    let host_platform =
        triple_to_platform(opts.host_platform.as_ref())?.or_else(|| target_platform.clone());
    let cargo_opts = CargoOptions::new()
        .with_dev_deps(opts.include_dev)
        .with_target_platform(target_platform.as_ref())
        .with_host_platform(host_platform.as_ref());
    println!("cargo opts: {:?}", cargo_opts);

    // TODO: allow package/feature/omitted selection
    let mut command = MetadataCommand::new();
    let pkg_graph = PackageGraph::from_command(&mut command)?;
    let feature_graph = pkg_graph.feature_graph();

    let query = if opts.packages.is_empty() {
        feature_graph.query_workspace(default_filter())
    } else {
        let pkg_ids = opts
            .packages
            .iter()
            .map(|name| pkg_graph.workspace().member_by_name(name).unwrap().id());
        let package_query = pkg_graph.query_forward(pkg_ids).expect("valid package IDs");
        feature_graph.query_packages(&package_query, default_filter())
    };

    let cargo_set = query.resolve_cargo(&cargo_opts)?;
    for (package, features) in cargo_set
        .target_features()
        .clone()
        .into_packages_with_features::<Vec<_>>(DependencyDirection::Forward)
    {
        println!("{}: {:?}", package.name(), features);
    }

    Ok(())
}

struct NameVisitor;

impl PackageDotVisitor for NameVisitor {
    fn visit_package(&self, package: &PackageMetadata, f: &mut DotWrite<'_, '_>) -> fmt::Result {
        write!(f, "{}", package.name())
    }

    fn visit_link(&self, _link: PackageLink<'_>, f: &mut DotWrite<'_, '_>) -> fmt::Result {
        write!(f, "")
    }
}

#[derive(Debug, StructOpt)]
pub struct CmdSelectOptions {
    #[structopt(flatten)]
    filter_opts: FilterOptions,

    #[structopt(long = "output-reverse", parse(from_flag = parse_direction))]
    /// Output results in reverse topological order (default: forward)
    output_direction: DependencyDirection,

    #[structopt(long, rename_all = "kebab-case")]
    /// Save selection graph in .dot format
    output_dot: Option<String>,

    #[structopt(flatten)]
    query_opts: QueryOptions,
}

pub fn cmd_select(options: &CmdSelectOptions) -> Result<(), anyhow::Error> {
    let mut command = MetadataCommand::new();
    let pkg_graph = PackageGraph::from_command(&mut command)?;

    // XXX generalize this!
    let query = options.query_opts.apply(&pkg_graph)?;

    let feature_graph = pkg_graph.feature_graph();
    let feature_query = feature_graph.query_packages(&query, default_filter());
    let resolver = options.filter_opts.make_feature_resolver(&pkg_graph);
    let feature_set = feature_query.resolve_with_fn(resolver);

    for (package, features) in feature_set
        .clone()
        .into_packages_with_features::<Vec<_>>(options.output_direction)
    {
        let in_workspace = package.in_workspace();
        let direct_dep = pkg_graph
            .reverse_dep_links(package.id())
            .unwrap()
            .any(|l| l.from.in_workspace() && !l.to.in_workspace());
        let show_package = match options.filter_opts.kind {
            Kind::All => true,
            Kind::Workspace => in_workspace,
            Kind::DirectThirdParty => direct_dep,
            Kind::ThirdParty => !in_workspace,
        };
        if show_package {
            println!("{}: {:?}", package.name(), features);
        }
    }

    if let Some(ref output_file) = options.output_dot {
        let package_set = feature_set.to_package_set();
        let dot = package_set.into_dot(NameVisitor);
        let mut f = fs::File::create(output_file)?;
        write!(f, "{}", dot)?;
    }

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct SubtreeSizeOptions {
    #[structopt(flatten)]
    filter_opts: FilterOptions,

    // TODO: potentially replace this with SelectOptions
    #[structopt(rename_all = "screaming_snake_case")]
    /// The root packages to start the selection from
    root: Option<String>,
}

pub fn cmd_subtree_size(options: &SubtreeSizeOptions) -> Result<(), anyhow::Error> {
    let mut command = MetadataCommand::new();
    let pkg_graph = PackageGraph::from_command(&mut command)?;

    let resolver = options.filter_opts.make_resolver(&pkg_graph);

    let mut dep_cache = pkg_graph.new_depends_cache();

    let root_id = options
        .root
        .as_ref()
        .and_then(|root_name| {
            pkg_graph
                .packages()
                .find(|metadata| root_name == metadata.name())
        })
        .map(|metadata| metadata.id());
    let selection = if options.root.is_some() {
        pkg_graph.query_forward(iter::once(root_id.unwrap()))?
    } else {
        pkg_graph.query_workspace()
    };

    let mut unique_deps: HashMap<&PackageId, HashSet<&PackageId>> = HashMap::new();
    for package_id in selection
        .resolve_with_fn(&resolver)
        .into_ids(DependencyDirection::Forward)
    {
        let subtree_package_set: HashSet<&PackageId> = pkg_graph
            .query_forward(iter::once(package_id))?
            .resolve_with_fn(&resolver)
            .into_ids(DependencyDirection::Forward)
            .collect();
        let mut nonunique_deps_set: HashSet<&PackageId> = HashSet::new();
        for dep_package_id in &subtree_package_set {
            // don't count ourself
            if *dep_package_id == package_id {
                continue;
            }

            let mut unique = true;
            for reverse_dep_link in pkg_graph.reverse_dep_links(dep_package_id).unwrap() {
                // skip build and dev dependencies
                if reverse_dep_link.edge.dev_only() {
                    continue;
                }

                if !subtree_package_set.contains(reverse_dep_link.from.id())
                    || nonunique_deps_set.contains(reverse_dep_link.from.id())
                {
                    // if the from is from outside the subtree rooted at root_id, we ignore it
                    if let Some(root_id) = root_id {
                        if !dep_cache.depends_on(root_id, reverse_dep_link.from.id())? {
                            continue;
                        }
                    }

                    unique = false;
                    nonunique_deps_set.insert(dep_package_id);
                    break;
                }
            }

            let unique_list = unique_deps.entry(package_id).or_insert_with(HashSet::new);
            if unique {
                unique_list.insert(dep_package_id);
            }
        }
    }

    let mut sorted_unique_deps = unique_deps.into_iter().collect::<Vec<_>>();
    sorted_unique_deps.sort_by_key(|a| cmp::Reverse(a.1.len()));

    for (package_id, deps) in sorted_unique_deps.iter() {
        if !deps.is_empty() {
            println!("{} {}", deps.len(), package_id);
        }
        for dep in deps {
            println!("    {}", dep);
        }
    }

    Ok(())
}
