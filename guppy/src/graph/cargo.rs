// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Simulations of Cargo behavior.
//!
//! Cargo comes with a set of algorithms to figure out what packages or features are built. This
//! module reimplements those algorithms using `guppy`'s data structures.

use crate::graph::feature::{all_filter, CrossLink, FeatureQuery, FeatureSet};
use crate::graph::{DependencyDirection, EnabledTernary, PackageIx, PackageLink};
use crate::sorted_set::SortedSet;
use crate::{DependencyKind, Error, PackageId};
use petgraph::prelude::*;
use std::collections::HashSet;
use target_spec::Platform;

/// Options for queries which simulate what Cargo does.
///
/// This provides control over the resolution algorithm used by `guppy`'s simulation of Cargo.
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

impl<'a> Default for CargoOptions<'a> {
    fn default() -> Self {
        Self::new()
    }
}

/// A set of packages and features, as would be built by Cargo.
///
/// Cargo implements a set of algorithms to figure out which packages or features are built in
/// a given situation. `guppy` implements those algorithms.
pub struct CargoSet<'g> {
    target_features: FeatureSet<'g>,
    host_features: FeatureSet<'g>,
    proc_macro_edge_ixs: SortedSet<EdgeIndex<PackageIx>>,
}

impl<'g> CargoSet<'g> {
    /// Creates a new `CargoSet` based on the given query and options.
    ///
    /// This is also accessible through `FeatureQuery::resolve_cargo()`, and it may be more
    /// convenient to use that if the code is written in a "fluent" style.
    pub fn new(query: FeatureQuery<'g>, opts: &CargoOptions<'_>) -> Result<Self, Error> {
        let build_state = CargoSetBuildState::new(&query, opts)?;
        Ok(build_state.build(query))
    }

    /// Returns the feature set enabled on the target platform.
    ///
    /// This represents the packages and features that are included as code in the final build
    /// artifacts. This is relevant for both cross-compilation and auditing.
    pub fn target_features(&self) -> &FeatureSet<'g> {
        &self.target_features
    }

    /// Returns the feature set enabled on the host platform.
    ///
    /// This represents the packages and features that influence the final build artifacts, but
    /// whose code is generally not directly included.
    pub fn host_features(&self) -> &FeatureSet<'g> {
        &self.host_features
    }

    /// Returns `PackageLink` instances for procedural macro dependencies from target packages.
    ///
    /// Procedural macros straddle the line between target and host: they're built for the host
    /// but generate code that is compiled for the target platform.
    ///
    /// ## Notes
    ///
    /// Procedural macro packages will be included in the *host* feature set.
    ///
    /// The returned iterator will include proc macros that are depended on normally or in dev
    /// builds from initials (if `include_dev` is set), but not the ones in the
    /// `[build-dependencies]` section.
    pub fn proc_macro_links<'a>(
        &'a self,
    ) -> impl Iterator<Item = PackageLink<'g>> + ExactSizeIterator + 'a {
        let package_graph = self.target_features.graph().package_graph;
        self.proc_macro_edge_ixs.iter().map(move |edge_ix| {
            let (source_ix, target_ix) = package_graph
                .dep_graph
                .edge_endpoints(*edge_ix)
                .expect("valid edge ix");
            package_graph.edge_to_link(source_ix, target_ix, *edge_ix, None)
        })
    }
}

struct CargoSetBuildState<'a> {
    opts: &'a CargoOptions<'a>,
    omitted_packages: SortedSet<NodeIndex<PackageIx>>,
}

impl<'a> CargoSetBuildState<'a> {
    fn new(query: &FeatureQuery<'_>, opts: &'a CargoOptions<'a>) -> Result<Self, Error> {
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

    fn build(self, query: FeatureQuery<'_>) -> CargoSet {
        match self.opts.version {
            CargoResolverVersion::V1 => self.new_v1(query, false),
            CargoResolverVersion::V1Install => {
                let avoid_dev_deps = !self.opts.include_dev;
                self.new_v1(query, avoid_dev_deps)
            }
            CargoResolverVersion::V2 => self.new_v2(query),
        }
    }

    fn new_v1(self, query: FeatureQuery<'_>, avoid_dev_deps: bool) -> CargoSet {
        // Prepare a package query for step 2.
        let graph = *query.graph();
        let package_ixs: SortedSet<_> = query
            .params
            .initials()
            .iter()
            .map(|feature_ix| graph.package_ix_for_feature_ix(*feature_ix))
            .collect();
        let target_query = graph
            .package_graph
            .query_from_parts(package_ixs, DependencyDirection::Forward);

        // 1. Perform a "complete" feature query. This will provide more packages than will be
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

        // While doing traversal 2 below, record any packages discovered along build edges for use
        // in step 3. This will also include proc-macros.
        let mut host_ixs = Vec::new();
        // This list will contain proc-macro edges out of normal or dev dependencies.
        let mut proc_macro_edge_ixs = Vec::new();

        let is_enabled =
            |link: PackageLink<'_>, kind: DependencyKind, platform: Option<&Platform<'_>>| {
                let (from, to) = link.endpoints();
                let req_status = link.req_for_kind(kind).status();
                // Check the complete set to figure out whether we look at required_on or
                // enabled_on.
                let consider_optional = complete_set
                    .contains((from.id(), link.dep_name()))
                    .unwrap_or_else(|| {
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

        // 2. Figure out what packages will be included on the target platform, i.e. normal + dev
        // (if requested).
        let target_packages = target_query.resolve_with_fn(|query, link| {
            let (from, to) = link.endpoints();

            if self.is_omitted(to.package_ix()) {
                // Pretend that the omitted set doesn't exist.
                return false;
            }

            let consider_dev =
                self.opts.include_dev && query.starts_from(from.id()).expect("valid ID");
            // Build dependencies are only considered if there's a build script.
            let consider_build = from.has_build_script();

            let mut follow_target =
                is_enabled(link, DependencyKind::Normal, self.opts.target_platform)
                    || (consider_dev
                        && is_enabled(
                            link,
                            DependencyKind::Development,
                            self.opts.target_platform,
                        ));

            // Proc macros build on the host, so for normal/dev dependencies redirect it to the host
            // instead.
            if follow_target && to.is_proc_macro() {
                host_ixs.push(to.package_ix());
                proc_macro_edge_ixs.push(link.edge_ix());
                follow_target = false;
            }

            // Build dependencies are evaluated against the host platform.
            if consider_build && is_enabled(link, DependencyKind::Build, self.opts.host_platform) {
                host_ixs.push(to.package_ix());
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
                let consider_build = from.has_build_script();

                // Only normal and build dependencies are considered, regardless of whether this is
                // an initial. (Dev-dependencies of initials would have been considered in step 2).
                is_enabled(link, DependencyKind::Normal, self.opts.host_platform)
                    || (consider_build
                        && is_enabled(link, DependencyKind::Build, self.opts.host_platform))
            });

        // Finally, the features are whatever packages were selected, intersected with whatever
        // features were selected.
        let target_features = graph
            .resolve_packages(&target_packages, all_filter())
            .intersection(&complete_set);
        let host_features = graph
            .resolve_packages(&host_packages, all_filter())
            .intersection(&complete_set);

        let proc_macro_edge_ixs = SortedSet::new(proc_macro_edge_ixs);

        CargoSet {
            target_features,
            host_features,
            proc_macro_edge_ixs,
        }
    }

    fn new_v2(self, query: FeatureQuery<'_>) -> CargoSet {
        let graph = *query.graph();

        let is_enabled = |link: CrossLink<'_>,
                          kind: DependencyKind,
                          platform: Option<&Platform<'_>>| {
            let platform_status = link.status_for_kind(kind);

            match platform {
                Some(platform) => platform_status.enabled_on(platform) != EnabledTernary::Disabled,
                None => !platform_status.is_never(),
            }
        };

        // State to maintain between steps 1 and 2.
        let mut host_ixs = Vec::new();
        let mut proc_macro_edge_ixs = Vec::new();

        // 1. Perform a feature query for the target.
        let target_features = query.clone().resolve_with_fn(|query, link| {
            let (from, to) = link.endpoints();

            if self.is_omitted(to.package_ix()) {
                // Pretend that the omitted set doesn't exist.
                return false;
            }

            let consider_dev =
                self.opts.include_dev && query.starts_from(from.feature_id()).expect("valid ID");
            // The V2 resolver behaves differently from the V1 resolver -- it doesn't appear to
            // check for whether this package has a build script.
            // XXX is this broken in upstream cargo?

            let mut follow_target =
                is_enabled(link, DependencyKind::Normal, self.opts.target_platform)
                    || (consider_dev
                        && is_enabled(
                            link,
                            DependencyKind::Development,
                            self.opts.target_platform,
                        ));

            // Proc macros build on the host, so for normal/dev dependencies redirect it to the host
            // instead.
            if follow_target && to.package().is_proc_macro() {
                host_ixs.push(to.feature_ix());
                proc_macro_edge_ixs.push(link.package_edge_ix());
                follow_target = false;
            }

            // Build dependencies are evaluated against the host platform.
            if is_enabled(link, DependencyKind::Build, self.opts.host_platform) {
                host_ixs.push(to.feature_ix());
            }

            follow_target
        });

        // 2. Perform a feature query for the host.
        let host_features = graph
            .query_from_parts(SortedSet::new(host_ixs), DependencyDirection::Forward)
            .resolve_with_fn(|_, link| {
                let (from, to) = link.endpoints();
                if self.is_omitted(to.package_ix()) {
                    // Pretend that the omitted set doesn't exist.
                    return false;
                }
                // The V2 resolver behaves differently from the V1 resolver -- it doesn't appear to
                // check for whether this package has a build script. It also unifies dev
                // dependencies of initials, even on the host platform.
                // XXX is this a bug in upstream cargo?
                let consider_dev = self.opts.include_dev
                    && query.starts_from(from.feature_id()).expect("valid ID");

                // Interestingly, dev-dependencies of initials may also be followed on the host
                // platform.
                // XXX is this a bug in upstream cargo?
                is_enabled(link, DependencyKind::Normal, self.opts.host_platform)
                    || is_enabled(link, DependencyKind::Build, self.opts.host_platform)
                    || (consider_dev
                        && is_enabled(link, DependencyKind::Development, self.opts.host_platform))
            });

        let proc_macro_edge_ixs = SortedSet::new(proc_macro_edge_ixs);

        CargoSet {
            target_features,
            host_features,
            proc_macro_edge_ixs,
        }
    }

    // ---
    // Helper methods
    // ---

    fn is_omitted(&self, package_ix: NodeIndex<PackageIx>) -> bool {
        self.omitted_packages.contains(&package_ix)
    }
}

/// The version of Cargo's feature resolver to use.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum CargoResolverVersion {
    /// The default "classic" feature resolver in Rust.
    ///
    /// This feature resolver unifies features across inactive targets, and also unifies features
    /// across normal, build and dev dependencies for initials. This may produce results that are
    /// surprising at times.
    V1,
    /// The "classic" feature resolver in Rust, as used by commands like `cargo install`.
    ///
    /// This resolver avoids unifying features across dev dependencies for initials. However, if
    /// `CargoOptions::with_dev_deps` is set to true, it behaves identically to the V1 resolver.
    ///
    /// For more, see
    /// [avoid-dev-deps](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#avoid-dev-deps)
    /// in the Cargo reference.
    V1Install,
    /// The new feature resolver.
    ///
    /// This is currently available as `-Zfeatures=all`, and is expected to be released in a future
    /// version of Cargo.
    ///
    /// For more, see
    /// [Features](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#features) in the
    /// Cargo reference.
    V2,
}
