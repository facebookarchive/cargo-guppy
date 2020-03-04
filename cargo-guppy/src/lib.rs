// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow;
use clap::arg_enum;
use guppy::{
    graph::{DependencyLink, DotWrite, PackageDotVisitor, PackageGraph, PackageMetadata},
    MetadataCommand, PackageId,
};
use itertools;
use std::cmp;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::io::Write;
use std::iter;
use structopt::StructOpt;
use target_spec;

mod diff;

// NOTE: There is currently a bug in guppy where it doesn't handle cycles
// well. We currently make use of into_iter_links() and stuffing the from/to
// packages in a HashSet to work around this.

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
    let mut pkg_graph = PackageGraph::from_command(&mut command)?;

    // narrow the graph
    narrow_graph(&mut pkg_graph, &filter_opts);

    let selection = pkg_graph.select_workspace();

    let mut dupe_map: HashMap<_, HashSet<_>> = HashMap::new();
    for link in selection.into_iter_links(None) {
        dupe_map
            .entry(link.from.name())
            .or_default()
            .insert(link.from.id());
        dupe_map
            .entry(link.to.name())
            .or_default()
            .insert(link.to.id());
    }

    for (name, dupes) in dupe_map {
        if dupes.len() <= 1 {
            continue;
        }

        let output = itertools::join(
            dupes
                .iter()
                .map(|p| pkg_graph.metadata(p).unwrap().version()),
            ", ",
        );

        println!("{} ({})", name, output);
    }

    Ok(())
}

arg_enum! {
    #[derive(Debug)]
    pub enum Kind {
        All,
        Workspace,
        DirectThirdParty,
        ThirdParty,
    }
}

struct NameVisitor;

impl PackageDotVisitor for NameVisitor {
    fn visit_package(&self, package: &PackageMetadata, mut f: DotWrite<'_, '_>) -> fmt::Result {
        write!(f, "{}", package.name())
    }

    fn visit_link(&self, _link: DependencyLink<'_>, mut f: DotWrite<'_, '_>) -> fmt::Result {
        write!(f, "")
    }
}

#[derive(Debug, StructOpt)]
pub struct SelectOptions {
    #[structopt(flatten)]
    filter_opts: FilterOptions,

    #[structopt(long, rename_all = "kebab-case")]
    /// Save selection graph in .dot format
    output_dot: Option<String>,

    #[structopt(rename_all = "screaming_snake_case")]
    /// The root packages to start the selection from
    roots: Vec<String>,
}

#[derive(Debug, StructOpt)]
pub struct FilterOptions {
    #[structopt(long, short, possible_values = &Kind::variants(), case_insensitive = true, default_value = "all")]
    /// Kind of crates to select
    kind: Kind,

    #[structopt(long, rename_all = "kebab-case")]
    /// Include dev dependencies
    include_dev: bool,

    #[structopt(long, rename_all = "kebab-case")]
    /// Include build dependencies
    include_build: bool,

    #[structopt(long)]
    /// Target to select for, default is to match all targets
    target: Option<String>,

    #[structopt(
        long,
        rename_all = "kebab-case",
        name = "package",
        number_of_values = 1
    )]
    /// Omit edges that point into a given package; useful for seeing how
    /// removing a dependency affects the graph
    omit_edges_into: Vec<String>,
}

pub fn cmd_select(options: &SelectOptions) -> Result<(), anyhow::Error> {
    let mut command = MetadataCommand::new();
    let mut pkg_graph = PackageGraph::from_command(&mut command)?;
    let mut package_ids = HashSet::new();
    let mut omitted_package_ids = HashSet::new();

    // NOTE: The root set packages are specified by name. The tool currently
    // does not handle multiple version of the same package as the current use
    // cases are passing workspace members as the root set, which won't be
    // duplicated.
    let root_set: HashSet<String> = if !options.roots.is_empty() {
        options.roots.iter().cloned().collect()
    } else {
        pkg_graph
            .workspace()
            .member_ids()
            .map(|pkg_id| pkg_graph.metadata(pkg_id).unwrap().name().to_string())
            .collect()
    };

    let omitted_set: HashSet<String> = options
        .filter_opts
        .omit_edges_into
        .iter()
        .cloned()
        .collect();

    for metadata in pkg_graph.packages() {
        if root_set.contains(metadata.name()) {
            package_ids.insert(metadata.id().clone());
        }
        if omitted_set.contains(metadata.name()) {
            omitted_package_ids.insert(metadata.id().clone());
        }
    }

    narrow_graph(&mut pkg_graph, &options.filter_opts);

    let mut selected_packages = HashSet::new();
    let mut direct_dependencies = HashSet::new();
    for link in pkg_graph
        .select_forward(&package_ids)?
        .into_iter_links(None)
    {
        selected_packages.insert(link.from.id());
        selected_packages.insert(link.to.id());
        if link.from.in_workspace() && !link.to.in_workspace() {
            direct_dependencies.insert(link.to.id());
        }
    }

    for package_id in selected_packages {
        let in_workspace = pkg_graph.metadata(package_id).unwrap().in_workspace();
        let show_package = match options.filter_opts.kind {
            Kind::All => true,
            Kind::Workspace => in_workspace,
            Kind::DirectThirdParty => direct_dependencies.contains(package_id),
            Kind::ThirdParty => !in_workspace,
        };
        if show_package {
            println!("{}", pkg_graph.metadata(package_id).unwrap().id());
        }
    }

    if let Some(ref output_file) = options.output_dot {
        let dot = pkg_graph
            .select_forward(&package_ids)?
            .into_dot(NameVisitor);
        let mut f = fs::File::create(output_file)?;
        write!(f, "{}", dot)?;
    }

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct SubtreeSizeOptions {
    #[structopt(flatten)]
    filter_opts: FilterOptions,

    #[structopt(rename_all = "screaming_snake_case")]
    /// The root packages to start the selection from
    root: Option<String>,
}

pub fn cmd_subtree_size(options: &SubtreeSizeOptions) -> Result<(), anyhow::Error> {
    let mut command = MetadataCommand::new();
    let mut pkg_graph = PackageGraph::from_command(&mut command)?;

    // narrow the graph
    narrow_graph(&mut pkg_graph, &options.filter_opts);

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
        pkg_graph.select_forward(iter::once(root_id.unwrap()))?
    } else {
        pkg_graph.select_workspace()
    };

    let mut unique_deps: HashMap<&PackageId, HashSet<&PackageId>> = HashMap::new();
    for package_id in selection.into_iter_ids(None) {
        let subtree_package_set: HashSet<&PackageId> = pkg_graph
            .select_forward(iter::once(package_id))?
            .into_iter_ids(None)
            .collect();
        let mut nonunique_deps_set: HashSet<&PackageId> = HashSet::new();
        for dep_package_id in pkg_graph
            .select_forward(iter::once(package_id))?
            .into_iter_ids(None)
        {
            // don't count ourself
            if dep_package_id == package_id {
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

/// Narrow a package graph by removing specific edges.
fn narrow_graph(pkg_graph: &mut PackageGraph, options: &FilterOptions) {
    let omitted_set: HashSet<String> = options.omit_edges_into.iter().cloned().collect();

    let mut omitted_package_ids = HashSet::new();
    for metadata in pkg_graph.packages() {
        if omitted_set.contains(metadata.name()) {
            omitted_package_ids.insert(metadata.id().clone());
        }
    }

    pkg_graph.retain_edges(|_, DependencyLink { from, to, edge }| {
        // filter by the kind of dependency (--kind)
        // NOTE: We always retain all workspace deps in the graph, otherwise
        // we'll get a disconnected graph.
        let include_kind = match options.kind {
            Kind::All | Kind::ThirdParty => true,
            Kind::DirectThirdParty => from.in_workspace(),
            Kind::Workspace => from.in_workspace() && to.in_workspace(),
        };

        // filter out irrelevant dependencies for a specific target (--target)
        let include_target = if let Some(ref target) = options.target {
            edge.normal()
                .and_then(|meta| meta.target())
                .and_then(|edge_target| {
                    let res = target_spec::eval(edge_target, target).unwrap_or(true);
                    Some(res)
                })
                .unwrap_or(true)
        } else {
            true
        };

        // filter normal, dev, and build dependencies (--include-build, --include-dev)
        let include_type = edge.normal().is_some()
            || options.include_dev && edge.dev().is_some()
            || options.include_build && edge.build().is_some();

        // filter out provided edge targets (--omit-edges-into)
        let include_edge = !omitted_package_ids.contains(to.id());

        include_kind && include_target && include_type && include_edge
    });
}
