// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A command-line frontend for `guppy`.
//!
//! `cargo-guppy` provides a frontend for running `guppy` queries.
//!
//! # Installing
//!
//! `cargo-guppy` is currently a work in progress, and not yet on `crates.io`. To install it, ensure
//! you have `cargo` installed (preferably through [rustup](https://rustup.rs/)), then run:
//!
//! ```bash
//! cargo install --git https://github.com/facebookincubator/cargo-guppy --branch main cargo-guppy
//! ```
//!
//! This will make the `cargo guppy` command available.
//!
//! # Commands
//!
//! The list of commands is not currently stable and is subject to change.
//!
//! ## Query commands
//!
//! * `select`: query packages and their transitive dependencies
//! * `resolve-cargo`: query packages and features as would be built by cargo
//! * `subtree-size`: print dependencies along with their unique subtree size
//! * `dups`: print duplicate packages
//!
//! ## Diff commands
//!
//! * `diff`: perform a diff of two `cargo metadata` JSON outputs
//! * `diff-summaries`: perform a diff of two [summaries](https://github.com/facebookincubator/cargo-guppy/tree/main/guppy-summaries)
//!
//! ## Workspace manipulations
//!
//! * `mv`: move crates to a new location in a workspace, updating paths along the way

mod core;
mod diff;
mod mv;

pub use crate::{core::*, mv::*};

use camino::Utf8PathBuf;
use clap::{ArgEnum, Parser};
use color_eyre::eyre::{bail, Result, WrapErr};
use guppy::{
    graph::{
        cargo::{CargoOptions, CargoSet},
        feature::{FeatureSet, StandardFeatures},
        summaries::Summary,
        DependencyDirection, DotWrite, PackageDotVisitor, PackageGraph, PackageLink,
        PackageMetadata,
    },
    PackageId,
};
use guppy_cmdlib::{
    string_to_platform_spec, CargoMetadataOptions, CargoResolverOpts, PackagesAndFeatures,
};
use std::{
    borrow::Cow,
    cmp,
    collections::{HashMap, HashSet},
    fmt, fs,
    io::Write,
    iter,
    path::PathBuf,
};

pub fn cmd_diff(json: bool, old: &str, new: &str) -> Result<()> {
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

#[derive(Debug, Parser)]
pub struct DiffSummariesOptions {
    /// The old summary
    #[clap(name = "OLD")]
    pub old: Utf8PathBuf,

    /// The new summary
    #[clap(name = "NEW")]
    pub new: Utf8PathBuf,
}

impl DiffSummariesOptions {
    pub fn exec(&self) -> Result<()> {
        let old_summary = fs::read_to_string(&self.old)
            .wrap_err_with(|| format!("reading old summary {} failed", self.old))?;
        let old_summary = Summary::parse(&old_summary)
            .wrap_err_with(|| format!("parsing old summary {} failed", self.old))?;

        let new_summary = fs::read_to_string(&self.new)
            .wrap_err_with(|| format!("reading new summary {} failed", self.new))?;
        let new_summary = Summary::parse(&new_summary)
            .wrap_err_with(|| format!("parsing new summary {} failed", self.new))?;

        let diff = old_summary.diff(&new_summary);

        println!("{}", diff.report());

        // TODO: different error codes for non-empty diff and failure, similar to git/hg
        if diff.is_changed() {
            bail!("non-empty diff");
        }
        Ok(())
    }
}

#[derive(Debug, Parser)]
pub struct DupsOptions {
    #[clap(flatten)]
    filter_opts: FilterOptions,

    #[clap(flatten)]
    metadata_opts: CargoMetadataOptions,
}

pub fn cmd_dups(opts: &DupsOptions) -> Result<()> {
    let command = opts.metadata_opts.make_command();
    let pkg_graph = command.build_graph()?;

    let resolver = opts.filter_opts.make_resolver(&pkg_graph)?;
    let selection = pkg_graph.query_workspace();

    let mut dupe_map: HashMap<_, Vec<_>> = HashMap::new();
    for package in selection
        .resolve_with_fn(resolver)
        .packages(DependencyDirection::Forward)
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

#[derive(ArgEnum, Copy, Clone, Debug)]
pub enum BuildKind {
    All,
    Target,
    ProcMacro,
    TargetAndProcMacro,
    Host,
}

#[derive(Debug, Parser)]
pub struct ResolveCargoOptions {
    #[clap(flatten)]
    pf: PackagesAndFeatures,

    #[clap(flatten)]
    resolver_opts: CargoResolverOpts,

    #[clap(flatten)]
    base_filter_opts: BaseFilterOptions,

    #[clap(long = "target-platform")]
    /// Evaluate against target platform, "current" or "any" (default: any)
    target_platform: Option<String>,

    #[clap(long = "host-platform")]
    /// Evaluate against host platform, "current" or "any" (default: any)
    host_platform: Option<String>,

    #[clap(long, arg_enum, default_value = "all")]
    /// Print packages built on target, host or both
    build_kind: BuildKind,

    #[clap(long, parse(from_os_str))]
    /// Write summary file
    summary: Option<PathBuf>,

    #[clap(flatten)]
    metadata_opts: CargoMetadataOptions,
}

pub fn cmd_resolve_cargo(opts: &ResolveCargoOptions) -> Result<()> {
    let target_platform = string_to_platform_spec(opts.target_platform.as_deref())?;
    let host_platform = string_to_platform_spec(opts.host_platform.as_deref())?;
    let command = opts.metadata_opts.make_command();
    let pkg_graph = command.build_graph()?;

    let mut cargo_opts = CargoOptions::new();
    cargo_opts
        .set_include_dev(opts.resolver_opts.include_dev)
        .set_resolver(opts.resolver_opts.resolver_version.to_guppy())
        .set_initials_platform(opts.resolver_opts.initials_platform.to_guppy())
        .set_target_platform(target_platform)
        .set_host_platform(host_platform)
        .add_omitted_packages(opts.base_filter_opts.omitted_package_ids(&pkg_graph));

    let (initials, features_only) = opts.pf.make_feature_sets(&pkg_graph)?;
    let cargo_set = CargoSet::new(initials, features_only, &cargo_opts)?;

    // Note that for the target+proc macro case, we unify direct deps here. This means that
    // direct deps of workspace proc macros (e.g. quote) will be included. This feels like it's
    // what's desired for this request.
    let direct_deps = match opts.build_kind {
        BuildKind::All | BuildKind::TargetAndProcMacro => Cow::Owned(
            cargo_set
                .host_direct_deps()
                .union(cargo_set.target_direct_deps()),
        ),
        BuildKind::Target => Cow::Borrowed(cargo_set.target_direct_deps()),
        BuildKind::Host | BuildKind::ProcMacro => Cow::Borrowed(cargo_set.host_direct_deps()),
    };

    let print_packages = |feature_set: &FeatureSet| {
        for feature_list in feature_set.packages_with_features(DependencyDirection::Forward) {
            let package = feature_list.package();
            let show_package = match opts.base_filter_opts.kind {
                Kind::All => true,
                Kind::Workspace => package.in_workspace(),
                Kind::DirectThirdParty => {
                    !package.in_workspace()
                        && direct_deps.contains(package.id()).expect("valid package")
                }
                Kind::ThirdParty => !package.in_workspace(),
            };
            if show_package {
                println!(
                    "{} {}: {}",
                    package.name(),
                    package.version(),
                    feature_list.display_features()
                );
            }
        }
    };

    let proc_macro_features = || {
        let proc_macro_ids = cargo_set.proc_macro_links().map(|link| link.to().id());
        let package_set = pkg_graph.resolve_ids(proc_macro_ids).expect("valid IDs");
        let feature_set = package_set.to_feature_set(StandardFeatures::All);
        cargo_set.host_features().intersection(&feature_set)
    };
    match opts.build_kind {
        BuildKind::All => {
            print_packages(&cargo_set.target_features().union(cargo_set.host_features()))
        }
        BuildKind::Target => print_packages(cargo_set.target_features()),
        BuildKind::ProcMacro => print_packages(&proc_macro_features()),
        BuildKind::TargetAndProcMacro => {
            print_packages(&cargo_set.target_features().union(&proc_macro_features()))
        }
        BuildKind::Host => print_packages(cargo_set.host_features()),
    }

    if let Some(summary_path) = &opts.summary {
        let summary = cargo_set.to_summary(&cargo_opts)?;
        let mut out = "# This summary file was @generated by cargo-guppy.\n\n".to_string();
        summary.write_to_string(&mut out)?;

        fs::write(summary_path, out)?;
    }

    Ok(())
}

struct NameVisitor;

impl PackageDotVisitor for NameVisitor {
    fn visit_package(&self, package: PackageMetadata<'_>, f: &mut DotWrite<'_, '_>) -> fmt::Result {
        write!(f, "{}", package.name())
    }

    fn visit_link(&self, _link: PackageLink<'_>, f: &mut DotWrite<'_, '_>) -> fmt::Result {
        write!(f, "")
    }
}

#[derive(Debug, Parser)]
pub struct CmdSelectOptions {
    #[clap(flatten)]
    filter_opts: FilterOptions,

    #[clap(long = "output-reverse", parse(from_flag = parse_direction))]
    /// Output results in reverse topological order (default: forward)
    output_direction: DependencyDirection,

    #[clap(long, rename_all = "kebab-case")]
    /// Save selection graph in .dot format
    output_dot: Option<String>,

    #[clap(flatten)]
    query_opts: QueryOptions,

    #[clap(flatten)]
    metadata_opts: CargoMetadataOptions,
}

pub fn cmd_select(options: &CmdSelectOptions) -> Result<()> {
    let mut command = options.metadata_opts.make_command();
    command.other_options(["--no-deps"]);
    let pkg_graph = command.build_graph()?;

    let query = options.query_opts.apply(&pkg_graph)?;
    let resolver = options.filter_opts.make_resolver(&pkg_graph)?;
    let package_set = query.resolve_with_fn(resolver);

    for package_id in package_set.package_ids(options.output_direction) {
        let package = pkg_graph.metadata(package_id).unwrap();
        let in_workspace = package.in_workspace();
        let direct_dep = package
            .reverse_direct_links()
            .any(|link| link.from().in_workspace() && !link.to().in_workspace());
        let show_package = match options.filter_opts.base_opts.kind {
            Kind::All => true,
            Kind::Workspace => in_workspace,
            Kind::DirectThirdParty => direct_dep,
            Kind::ThirdParty => !in_workspace,
        };
        if show_package {
            println!("{}", package_id);
        }
    }

    if let Some(ref output_file) = options.output_dot {
        let dot = package_set.display_dot(NameVisitor);
        let mut f = fs::File::create(output_file)?;
        write!(f, "{}", dot)?;
    }

    Ok(())
}

#[derive(Debug, Parser)]
pub struct SubtreeSizeOptions {
    #[clap(flatten)]
    filter_opts: FilterOptions,

    // TODO: potentially replace this with SelectOptions
    #[clap(rename_all = "screaming_snake_case")]
    /// The root packages to start the selection from
    root: Option<String>,

    #[clap(flatten)]
    metadata_opts: CargoMetadataOptions,
}

pub fn cmd_subtree_size(options: &SubtreeSizeOptions) -> Result<()> {
    let command = options.metadata_opts.make_command();
    let pkg_graph = command.build_graph()?;

    let resolver = options.filter_opts.make_resolver(&pkg_graph)?;

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
        .package_ids(DependencyDirection::Forward)
    {
        let subtree_package_set: HashSet<&PackageId> = pkg_graph
            .query_forward(iter::once(package_id))?
            .resolve_with_fn(&resolver)
            .package_ids(DependencyDirection::Forward)
            .collect();
        let mut nonunique_deps_set: HashSet<&PackageId> = HashSet::new();
        for dep_package_id in &subtree_package_set {
            // don't count ourself
            if *dep_package_id == package_id {
                continue;
            }

            let mut unique = true;
            let dep_package = pkg_graph.metadata(dep_package_id).unwrap();
            for link in dep_package.reverse_direct_links() {
                // skip build and dev dependencies
                if link.dev_only() {
                    continue;
                }
                let from_id = link.from().id();

                if !subtree_package_set.contains(from_id) || nonunique_deps_set.contains(from_id) {
                    // if the from is from outside the subtree rooted at root_id, we ignore it
                    if let Some(root_id) = root_id {
                        if !dep_cache.depends_on(root_id, from_id)? {
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
