// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::cargo::{CargoIntermediateSet, CargoOptions, CargoResolverVersion, CargoSet};
use crate::graph::feature::{all_filter, CrossLink, FeatureQuery, FeatureSet};
use crate::graph::{DependencyDirection, EnabledTernary, PackageIx, PackageLink, PackageSet};
use crate::sorted_set::SortedSet;
use crate::{DependencyKind, Error};
use fixedbitset::FixedBitSet;
use once_cell::sync::OnceCell;
use petgraph::prelude::*;
use petgraph::visit::VisitMap;
use target_spec::Platform;

pub(super) struct CargoSetBuildState<'a> {
    opts: &'a CargoOptions<'a>,
    omitted_packages: SortedSet<NodeIndex<PackageIx>>,
}

impl<'a> CargoSetBuildState<'a> {
    pub(super) fn new<'g>(
        query: &FeatureQuery<'g>,
        opts: &'a CargoOptions<'a>,
    ) -> Result<Self, Error> {
        if query.direction() == DependencyDirection::Reverse {
            return Err(Error::CargoSetError(
                "attempted to compute for reverse query".into(),
            ));
        }

        let omitted_packages: SortedSet<_> = query
            .graph()
            .package_graph
            .package_ixs(opts.omitted_packages.iter().copied())?;

        Ok(Self {
            opts,
            omitted_packages,
        })
    }

    pub(super) fn build(self, query: FeatureQuery) -> CargoSet {
        match self.opts.version {
            CargoResolverVersion::V1 => self.new_v1(query, false),
            CargoResolverVersion::V1Install => {
                let avoid_dev_deps = !self.opts.include_dev;
                self.new_v1(query, avoid_dev_deps)
            }
            CargoResolverVersion::V2 => self.new_v2(query),
        }
    }

    pub(super) fn build_intermediate(self, query: FeatureQuery) -> CargoIntermediateSet {
        match self.opts.version {
            CargoResolverVersion::V1 => self.new_v1_intermediate(query, false),
            CargoResolverVersion::V1Install => {
                let avoid_dev_deps = !self.opts.include_dev;
                self.new_v1_intermediate(query, avoid_dev_deps)
            }
            CargoResolverVersion::V2 => self.new_v2_intermediate(query),
        }
    }

    fn new_v1(self, query: FeatureQuery, avoid_dev_deps: bool) -> CargoSet {
        self.build_set(query, |query| {
            self.new_v1_intermediate(query, avoid_dev_deps)
        })
    }

    fn new_v2(self, query: FeatureQuery) -> CargoSet {
        self.build_set(query, |query| self.new_v2_intermediate(query))
    }

    // ---
    // Helper methods
    // ---

    fn is_omitted(&self, package_ix: NodeIndex<PackageIx>) -> bool {
        self.omitted_packages.contains(&package_ix)
    }

    fn build_set<'g>(
        &self,
        original_query: FeatureQuery<'g>,
        intermediate_fn: impl FnOnce(FeatureQuery<'g>) -> CargoIntermediateSet<'g>,
    ) -> CargoSet<'g> {
        // Prepare a package query for step 2.
        let graph = *original_query.graph();
        // Note that currently, proc macros specified in initials are built on both the target and
        // the host.
        let mut host_ixs = Vec::new();
        let target_ixs: Vec<_> = original_query
            .params
            .initials()
            .iter()
            .filter_map(|feature_ix| {
                let metadata = graph.metadata_for_ix(*feature_ix);
                let package_ix = metadata.package_ix();
                if metadata.package().is_proc_macro() {
                    // Proc macros are built on the host.
                    host_ixs.push(package_ix);
                    if self.opts.proc_macros_on_target {
                        Some(package_ix)
                    } else {
                        None
                    }
                } else {
                    Some(package_ix)
                }
            })
            .collect();
        let target_query = graph
            .package_graph
            .query_from_parts(SortedSet::new(target_ixs), DependencyDirection::Forward);

        // 1. Build the intermediate set containing the features for any possible package that can
        // be built.
        let intermediate_set = intermediate_fn(original_query.clone());
        let (target_set, host_set) = intermediate_set.target_host_sets();

        // While doing traversal 2 below, record any packages discovered along build edges for use
        // in host ixs, to prepare for step 3. This will also include proc-macros.

        // This list will contain proc-macro edges out of target packages.
        let mut proc_macro_edge_ixs = Vec::new();
        // This list will contain build dep edges out of target packages.
        let mut build_dep_edge_ixs = Vec::new();

        let is_enabled = |feature_set: &FeatureSet<'_>,
                          link: &PackageLink<'_>,
                          kind: DependencyKind,
                          platform: Option<&Platform<'_>>| {
            let (from, to) = link.endpoints();
            let req_status = link.req_for_kind(kind).status();
            // Check the complete set to figure out whether we look at required_on or
            // enabled_on.
            let consider_optional = feature_set
                .contains((from.id(), link.dep_name()))
                .unwrap_or_else(|_| {
                    // If the feature ID isn't present, it means the dependency wasn't declared
                    // as optional. In that case the value doesn't matter.
                    debug_assert!(
                        req_status.optional_status().is_never(),
                        "for {} -> {}, dep '{}' not declared as optional",
                        from.name(),
                        to.name(),
                        link.dep_name()
                    );
                    false
                });

            match (consider_optional, platform) {
                (true, Some(platform)) => {
                    req_status.enabled_on(platform) != EnabledTernary::Disabled
                }
                (true, None) => req_status.enabled_on_any(),
                (false, Some(platform)) => {
                    req_status.required_on(platform) != EnabledTernary::Disabled
                }
                (false, None) => req_status.required_on_any(),
            }
        };

        // Record workspace + direct third-party deps in these sets.
        let mut target_direct_deps =
            FixedBitSet::with_capacity(graph.package_graph.package_count());
        let mut host_direct_deps = FixedBitSet::with_capacity(graph.package_graph.package_count());

        // 2. Figure out what packages will be included on the target platform, i.e. normal + dev
        // (if requested).
        let target_platform = self.opts.target_platform();
        let host_platform = self.opts.host_platform();

        let target_packages = target_query.resolve_with_fn(|query, link| {
            let (from, to) = link.endpoints();

            if from.in_workspace() {
                // Mark initials in target_direct_deps.
                target_direct_deps.visit(from.package_ix());
            }

            if self.is_omitted(to.package_ix()) {
                // Pretend that the omitted set doesn't exist.
                return false;
            }

            let consider_dev =
                self.opts.include_dev && query.starts_from(from.id()).expect("valid ID");
            // Build dependencies are only considered if there's a build script.
            let consider_build = from.has_build_script();

            let mut follow_target =
                is_enabled(target_set, &link, DependencyKind::Normal, target_platform)
                    || (consider_dev
                        && is_enabled(
                            target_set,
                            &link,
                            DependencyKind::Development,
                            target_platform,
                        ));

            // Proc macros build on the host, so for normal/dev dependencies redirect it to the host
            // instead.
            let proc_macro_redirect = follow_target && to.is_proc_macro();

            // Build dependencies are evaluated against the host platform.
            let build_dep_redirect = consider_build
                && is_enabled(target_set, &link, DependencyKind::Build, host_platform);

            // Finally, process what needs to be done.
            if build_dep_redirect || proc_macro_redirect {
                if from.in_workspace() {
                    // The 'to' node is either in the workspace or a direct dependency [a].
                    host_direct_deps.visit(to.package_ix());
                }
                host_ixs.push(to.package_ix());
            }
            if build_dep_redirect {
                build_dep_edge_ixs.push(link.edge_ix());
            }
            if proc_macro_redirect {
                proc_macro_edge_ixs.push(link.edge_ix());
                follow_target = false;
            }

            if from.in_workspace() && follow_target {
                // The 'to' node is either in the workspace or a direct dependency.
                target_direct_deps.visit(to.package_ix());
            }

            follow_target
        });

        // 3. Figure out what packages will be included on the host platform.
        let host_ixs = SortedSet::new(host_ixs);
        let host_packages = graph
            .package_graph
            .query_from_parts(host_ixs, DependencyDirection::Forward)
            .resolve_with_fn(|_, link| {
                let (from, to) = link.endpoints();
                if self.is_omitted(to.package_ix()) {
                    // Pretend that the omitted set doesn't exist.
                    return false;
                }

                // All relevant nodes in host_ixs have already been added to host_direct_deps at [a].

                let consider_build = from.has_build_script();

                // Only normal and build dependencies are considered, regardless of whether this is
                // an initial. (Dev-dependencies of initials would have been considered in step 2).
                let res = is_enabled(host_set, &link, DependencyKind::Normal, host_platform)
                    || (consider_build
                        && is_enabled(host_set, &link, DependencyKind::Build, host_platform));

                if res {
                    if from.in_workspace() {
                        // The 'to' node is either in the workspace or a direct dependency.
                        host_direct_deps.visit(to.package_ix());
                    }
                    true
                } else {
                    false
                }
            });

        // Finally, the features are whatever packages were selected, intersected with whatever
        // features were selected.
        let target_features = graph
            .resolve_packages(&target_packages, all_filter())
            .intersection(target_set);
        let host_features = graph
            .resolve_packages(&host_packages, all_filter())
            .intersection(host_set);

        // Also construct the direct dep sets.
        let target_direct_deps =
            PackageSet::from_included(graph.package_graph(), target_direct_deps);
        let host_direct_deps = PackageSet::from_included(graph.package_graph, host_direct_deps);

        CargoSet {
            original_query,
            target_features,
            host_features,
            target_direct_deps,
            host_direct_deps,
            proc_macro_edge_ixs: SortedSet::new(proc_macro_edge_ixs),
            build_dep_edge_ixs: SortedSet::new(build_dep_edge_ixs),

            unified_features: OnceCell::new(),
            unified_direct_deps: OnceCell::new(),
        }
    }

    fn new_v1_intermediate<'g>(
        &self,
        query: FeatureQuery<'g>,
        avoid_dev_deps: bool,
    ) -> CargoIntermediateSet<'g> {
        // Perform a "complete" feature query. This will provide more packages than will be
        // included in the final build, but for each package it will have the correct feature set.
        let complete_set = query.resolve_with_fn(|query, link| {
            if self.is_omitted(link.to().package_ix()) {
                // Pretend that the omitted set doesn't exist.
                false
            } else if !avoid_dev_deps
                && query
                    .starts_from(link.from().feature_id())
                    .expect("valid ID")
            {
                // Follow everything for initials.
                true
            } else {
                // Follow normal and build edges for everything else.
                !link.dev_only()
            }
        });

        CargoIntermediateSet::Unified(complete_set)
    }

    fn new_v2_intermediate<'g>(&self, query: FeatureQuery<'g>) -> CargoIntermediateSet<'g> {
        let graph = *query.graph();
        // Note that proc macros specified in initials take part in feature resolution
        // for both target and host ixs. If they didn't, then the query would be partitioned into
        // host and target ixs instead.
        // https://github.com/rust-lang/cargo/issues/8312
        let mut host_ixs: Vec<_> = query
            .params
            .initials()
            .iter()
            .filter_map(|feature_ix| {
                let metadata = graph.metadata_for_ix(*feature_ix);
                if metadata.package().is_proc_macro() {
                    // Proc macros are built on the host.
                    Some(metadata.feature_ix())
                } else {
                    // Everything else is built on the target.
                    None
                }
            })
            .collect();

        let is_enabled = |link: &CrossLink<'_>,
                          kind: DependencyKind,
                          platform: Option<&Platform<'_>>| {
            let platform_status = link.status_for_kind(kind);

            match platform {
                Some(platform) => platform_status.enabled_on(platform) != EnabledTernary::Disabled,
                None => !platform_status.is_never(),
            }
        };

        // Keep a copy of the target query for use in step 2.
        let target_query = query.clone();

        // 1. Perform a feature query for the target.
        let target_platform = self.opts.target_platform();
        let host_platform = self.opts.host_platform();
        let target = query.resolve_with_fn(|query, link| {
            let (from, to) = link.endpoints();

            if self.is_omitted(to.package_ix()) {
                // Pretend that the omitted set doesn't exist.
                return false;
            }

            let consider_dev =
                self.opts.include_dev && query.starts_from(from.feature_id()).expect("valid ID");
            // This resolver doesn't check for whether this package has a build script.
            let mut follow_target = is_enabled(&link, DependencyKind::Normal, target_platform)
                || (consider_dev
                    && is_enabled(&link, DependencyKind::Development, target_platform));

            // Proc macros build on the host, so for normal/dev dependencies redirect it to the host
            // instead.
            let proc_macro_redirect = follow_target && to.package().is_proc_macro();

            // Build dependencies are evaluated against the host platform.
            let build_dep_redirect = is_enabled(&link, DependencyKind::Build, host_platform);

            // Finally, process what needs to be done.
            if build_dep_redirect || proc_macro_redirect {
                host_ixs.push(to.feature_ix());
            }
            if proc_macro_redirect {
                follow_target = false;
            }

            follow_target
        });

        // 2. Perform a feature query for the host.
        let host = graph
            .query_from_parts(SortedSet::new(host_ixs), DependencyDirection::Forward)
            .resolve_with_fn(|_, link| {
                let (from, to) = link.endpoints();
                if self.is_omitted(to.package_ix()) {
                    // Pretend that the omitted set doesn't exist.
                    return false;
                }
                // During feature resolution, the v2 resolver doesn't check for whether this package
                // has a build script. It also unifies dev dependencies of initials, even on the
                // host platform.
                let consider_dev = self.opts.include_dev
                    && target_query
                        .starts_from(from.feature_id())
                        .expect("valid ID");

                is_enabled(&link, DependencyKind::Normal, host_platform)
                    || is_enabled(&link, DependencyKind::Build, host_platform)
                    || (consider_dev
                        && is_enabled(&link, DependencyKind::Development, host_platform))
            });

        CargoIntermediateSet::TargetHost { target, host }
    }
}
