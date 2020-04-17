// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::feature::{all_filter, CrossLink, FeatureQuery, FeatureResolver, FeatureSet};
use crate::graph::{
    DependencyDirection, EnabledTernary, PackageIx, PackageLink, PackageResolver, PackageSet,
};
use crate::sorted_set::SortedSet;
use crate::{DependencyKind, Error, PackageId};
use petgraph::prelude::*;
use std::collections::HashSet;
use target_spec::Platform;

/// Options for queries which simulate what Cargo does.
///
/// This provides control over the exact resolution strategies used by Cargo.
#[derive(Clone, Debug)]
pub struct CargoOptions<'a> {
    version: CargoResolverVersion,
    include_dev: bool,
    host_platform: Option<&'a Platform<'a>>,
    target_platform: Option<&'a Platform<'a>>,
    omitted_packages: HashSet<&'a PackageId>,
}

impl<'a> CargoOptions<'a> {
    /// Creates a new `CargoOptions` with this resolver version and default settings.
    ///
    /// The default settings are similar to what a plain `cargo build` does:
    ///
    /// * use version 1 of the Cargo resolver
    /// * exclude dev-dependencies
    /// * resolve dependencies assuming any possible host or target platform
    /// * do not omit any packages.
    pub fn new() -> Self {
        Self {
            version: CargoResolverVersion::V1,
            include_dev: false,
            host_platform: None,
            target_platform: None,
            omitted_packages: HashSet::new(),
        }
    }

    /// Sets the Cargo feature resolver version.
    ///
    /// For more about feature resolution, see the documentation for `CargoResolverVersion`.
    pub fn with_version(mut self, version: CargoResolverVersion) -> Self {
        self.version = version;
        self
    }

    /// If set to true, causes dev-dependencies of the initial set to be followed.
    ///
    /// This does not affect transitive dependencies -- for example, a build or dev-dependency's
    /// further dev-dependencies are never followed.
    ///
    /// The default is true, which matches what a plain `cargo build` does.
    pub fn with_dev_deps(mut self, include_dev: bool) -> Self {
        self.include_dev = include_dev;
        self
    }

    /// Sets both the target and host platforms to the provided one, or to evaluate against any
    /// platform if `None`.
    pub fn with_platform(mut self, platform: Option<&'a Platform<'a>>) -> Self {
        self.target_platform = platform;
        self.host_platform = platform;
        self
    }

    /// Sets the target platform to the provided one, or to evaluate against any platform if `None`.
    pub fn with_target_platform(mut self, target_platform: Option<&'a Platform<'a>>) -> Self {
        self.target_platform = target_platform;
        self
    }

    /// Sets the host platform to the provided one, or to evaluate against any platform if `None`.
    pub fn with_host_platform(mut self, host_platform: Option<&'a Platform<'a>>) -> Self {
        self.host_platform = host_platform;
        self
    }

    /// Omits edges into the given packages.
    ///
    /// This may be useful in order to figure out what additional dependencies or features a
    /// particular set of packages pulls in.
    ///
    /// This method is additive.
    pub fn with_omitted_packages(
        mut self,
        package_ids: impl IntoIterator<Item = &'a PackageId>,
    ) -> Self {
        self.omitted_packages.extend(package_ids);
        self
    }
}

pub struct CargoSet<'g> {
    target_features: FeatureSet<'g>,
}

impl<'g> CargoSet<'g> {
    /// Returns the set of features enabled on the target.
    pub fn target_features(&self) -> &FeatureSet<'g> {
        &self.target_features
    }

    pub(super) fn new(query: FeatureQuery<'g>, opts: &CargoOptions<'_>) -> Result<Self, Error> {
        if query.direction() == DependencyDirection::Reverse {
            return Err(Error::CargoSetError(
                "attempted to compute for reverse query".into(),
            ));
        }

        let omitted_packages: SortedSet<_> = query
            .graph
            .package_graph
            .package_ixs(opts.omitted_packages.iter().copied())?;

        match opts.version {
            CargoResolverVersion::V1 => Ok(Self::new_v1(query, opts, omitted_packages)),
        }
    }

    fn new_v1(
        query: FeatureQuery<'g>,
        opts: &CargoOptions<'_>,
        omitted_packages: SortedSet<NodeIndex<PackageIx>>,
    ) -> Self {
        // Prepare a package query for step 2.
        let graph = query.graph;
        let package_ixs: SortedSet<_> = query
            .params
            .initials()
            .iter()
            .map(|feature_ix| graph.package_ix_for_feature_ix(*feature_ix))
            .collect();
        let package_query = graph
            .package_graph
            .query_from_parts(package_ixs, DependencyDirection::Forward);

        // 1. Perform a "complete" feature query. This will provide more packages than will be
        // included in the final build, but for each package it will have the correct feature set.
        let complete_set = query.resolve_with_fn(|query, link| {
            if query.starts_from(link.from.feature_id()).expect("valid ID") {
                // Follow everything for initials.
                true
            } else {
                // Follow normal and build edges for everything else.
                !link.edge.dev_only()
            }
        });

        // While doing traversal 2 below, record any packages discovered along build edges for use
        // in step 3. This will also include proc-macros.
        let mut host_ixs = Vec::new();
        // This list will contain proc-macro edges out of normal or dev dependencies.
        let mut proc_macro_edge_ixs = Vec::new();

        let is_enabled =
            |link: PackageLink<'_>, kind: DependencyKind, platform: Option<&Platform<'_>>| {
                let req_status = link.edge.req_for_kind(kind).status();
                // Check the complete set to figure out whether we look at required_on or
                // enabled_on.
                let consider_optional = complete_set
                    .contains((link.from.id(), link.edge.dep_name()))
                    .unwrap_or_else(|| {
                        // If the feature ID isn't present, it means the dependency wasn't declared
                        // as optional. In that case the value doesn't matter.
                        debug_assert!(
                            req_status.optional_status().is_never(),
                            "for {} -> {}, dep '{}' not declared as optional",
                            link.from.name(),
                            link.to.name(),
                            link.edge.dep_name()
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

        // 2. Figure out what packages will be included on the target platform, i.e. normal + dev
        // (if requested).
        let target_packages = package_query.resolve_with_fn(|query, link| {
            let consider_dev =
                opts.include_dev && query.starts_from(link.from.id()).expect("valid ID");
            // Build dependencies are only considered if there's a build script.
            let consider_build = link.from.has_build_script();

            let mut follow_target = is_enabled(link, DependencyKind::Normal, opts.target_platform)
                || (consider_dev
                    && is_enabled(link, DependencyKind::Development, opts.target_platform));

            // If the target is a proc-macro, redirect it to the host instead.
            if follow_target && link.to.is_proc_macro() {
                host_ixs.push(link.to.package_ix);
                proc_macro_edge_ixs.push(link.edge.edge_ix());
                follow_target = false;
            }

            // Build dependencies are evaluated against the host platform.
            if consider_build && is_enabled(link, DependencyKind::Build, opts.host_platform) {
                host_ixs.push(link.to.package_ix);
            }

            follow_target
        });

        // TODO: step 3

        // Finally, the target features are whatever packages were selected, intersected with
        // whatever features were selected.
        let target_features = graph
            .resolve_packages(&target_packages, all_filter())
            .intersection(&complete_set);

        Self { target_features }
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum CargoResolverVersion {
    V1,
}
