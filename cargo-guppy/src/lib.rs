// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow;
use clap::arg_enum;
use guppy::{
    graph::{
        DependencyDirection, DependencyLink, DotWrite, PackageDotVisitor, PackageGraph,
        PackageMetadata,
    },
    MetadataCommand,
};
use lockfile::{diff, lockfile::Lockfile};
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io::Write;
use structopt::StructOpt;
use target_spec;

pub fn cmd_diff(json: bool, old: &str, new: &str) -> Result<(), anyhow::Error> {
    let old = Lockfile::from_file(old)?;
    let new = Lockfile::from_file(new)?;

    let diff = diff::DiffOptions::default().diff(&old, &new);

    if json {
        println!("{}", serde_json::to_string_pretty(&diff).unwrap());
    } else {
        print!("{}", diff);
    }

    Ok(())
}

pub fn cmd_count() -> Result<(), anyhow::Error> {
    let lockfile = Lockfile::from_file("Cargo.lock")?;

    println!("Third-party Packages: {}", lockfile.third_party_packages());

    Ok(())
}

pub fn cmd_dups() -> Result<(), anyhow::Error> {
    let lockfile = Lockfile::from_file("Cargo.lock")?;

    lockfile.duplicate_packages();

    Ok(())
}

arg_enum! {
    #[derive(Debug)]
    pub enum Kind {
        All,
        Workspace,
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
struct FilterOptions {
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
    let root_set: HashSet<String> = if options.roots.len() > 0 {
        options.roots.iter().cloned().collect()
    } else {
        pkg_graph
            .select_reverse(pkg_graph.workspace().member_ids())?
            .into_root_metadatas(DependencyDirection::Forward)
            .map(|meta| meta.name().to_string())
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

    pkg_graph.retain_edges(|_, DependencyLink { from, to, edge }| {
        // filter by the kind of dependency (--kind)
        let include_kind = match options.filter_opts.kind {
            Kind::All | Kind::ThirdParty => true,
            Kind::Workspace => from.in_workspace() && to.in_workspace(),
        };

        // filter out irrelevant dependencies for a specific target (--target)
        let include_target = if let Some(ref target) = options.filter_opts.target {
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
            || options.filter_opts.include_dev && edge.dev().is_some()
            || options.filter_opts.include_build && edge.build().is_some();

        // filter out provided edge targets (--omit-edges-into)
        let include_edge = !omitted_package_ids.contains(to.id());

        include_kind && include_target && include_type && include_edge
    });

    for package in pkg_graph.select_forward(&package_ids)?.into_iter_ids(None) {
        let in_workspace = pkg_graph.metadata(package).unwrap().in_workspace();
        let show_package = match options.filter_opts.kind {
            Kind::All => true,
            Kind::Workspace => in_workspace,
            Kind::ThirdParty => !in_workspace,
        };
        if show_package {
            println!("{}", pkg_graph.metadata(package).unwrap().id());
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
