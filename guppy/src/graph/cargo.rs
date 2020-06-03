// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Simulations of Cargo behavior.
//!
//! Cargo comes with a set of algorithms to figure out what packages or features are built. This
//! module reimplements those algorithms using `guppy`'s data structures.

use crate::graph::feature::{all_filter, CrossLink, FeatureQuery, FeatureSet};
use crate::graph::{DependencyDirection, EnabledTernary, PackageIx, PackageLink, PackageQuery};
use crate::sorted_set::SortedSet;
use crate::{DependencyKind, Error, PackageId};
use petgraph::prelude::*;
use std::collections::HashSet;
use target_spec::Platform;

/// Options for queries which simulate what Cargo does.
///
/// This provides control over the resolution algorithm used by `guppy`'s simulation of Cargo.
#[derive(Clone, Debug)]
pub struct CargoOptions<'a, PF = ()> {
    version: CargoResolverVersion,
    include_dev: bool,
    proc_macros_on_target: bool,
    host_platform: Option<&'a Platform<'a>>,
    target_platform: Option<&'a Platform<'a>>,
    omitted_packages: HashSet<&'a PackageId>,
    postfilter: PF,
}

impl<'a> CargoOptions<'a> {
    /// Creates a new `CargoOptions` with this resolver version and default settings.
    ///
    /// The default settings are similar to what a plain `cargo build` does:
    ///
    /// * use version 1 of the Cargo resolver
    /// * exclude dev-dependencies
    /// * do not build proc macros specified in the query on the target platform
    /// * resolve dependencies assuming any possible host or target platform
    /// * do not omit any packages.
    pub fn new() -> Self {
        Self::new_postfilter(())
    }
}

impl<'g, 'a, F> CargoOptions<'a, PostfilterFn<F>>
where
    F: FnMut(CargoResolvePhase<'g, '_>, PackageLink<'g>) -> bool,
{
    /// Creates a new `CargoOptions` with the specified postfilter function.
    ///
    /// The default settings are the same as `CargoOptions::new`.
    ///
    /// A link is traversed if it otherwise meets all other requirements and if the postfilter
    /// returns true for it.
    pub fn new_postfilter_fn(f: F) -> Self {
        Self::new_postfilter(PostfilterFn::new(f))
    }
}

impl<'g, 'a, PF> CargoOptions<'a, PF>
where
    PF: CargoPostfilter<'g>,
{
    /// Creates a new `CargoOptions` with a specified postfilter.
    ///
    /// The default settings are the same as `CargoOptions::new`.
    ///
    /// A link is traversed if it otherwise meets all other requirements and if the postfilter
    /// returns true for it.
    pub fn new_postfilter(postfilter: PF) -> Self {
        CargoOptions {
            version: CargoResolverVersion::V1,
            include_dev: false,
            proc_macros_on_target: false,
            host_platform: None,
            target_platform: None,
            omitted_packages: HashSet::new(),
            postfilter,
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

    /// If set to true, causes procedural macros (and transitive dependencies) specified in the
    /// initial set to be built on the host platform as well, not just the target platform.
    ///
    /// Procedural macros are typically not built on the target platform, but if they contain binary
    /// or test targets they will be.
    ///
    /// Procedural macros that are dependencies of the initial set will only be built on the host
    /// platform, regardless of whether this configuration is set.
    pub fn with_proc_macros_on_target(mut self, proc_macros_on_target: bool) -> Self {
        self.proc_macros_on_target = proc_macros_on_target;
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

/// The phase of Cargo resolution currently being computed.
///
/// This is used in `CargoPostfilter`.
#[derive(Copy, Clone, Debug)]
pub enum CargoResolvePhase<'g, 'a> {
    /// A resolution phase that doesn't distinguish between host and target features.
    ///
    /// This is passed into `CargoPostfilter::accept_feature` for the `V1` and
    /// `V1Install` resolvers.
    V1Unified(&'a FeatureQuery<'g>),

    /// A resolution phase for packages on the target platform.
    ///
    /// This is passed into `CargoPostfilter::accept_package` for all resolvers.
    TargetPackage(&'a PackageQuery<'g>),

    /// A resolution phase for features on the target platform.
    ///
    /// This is passed into `CargoPostfilter::accept_feature` for the `V2` resolver.
    TargetFeature(&'a FeatureQuery<'g>),

    /// A resolution phase for packages on the host platform.
    ///
    /// This is passed into `CargoPostfilter::accept_package` for all resolvers.
    HostPackage(&'a PackageQuery<'g>),

    /// A resolution phase for features on the host platform.
    ///
    /// This is passed into `CargoPostfilter::accept_feature` for the `V2` resolver.
    HostFeature(&'a FeatureQuery<'g>),
}

/// A final filter for if a package or feature link should be traversed during Cargo resolution.
///
/// This may be useful for more advanced control over package and feature resolution.
pub trait CargoPostfilter<'g> {
    /// Returns true if this package link should be considered during a resolve operation.
    ///
    /// This is called for the `V1` and `V1Install` resolvers, and as part of the default
    /// implementation for `accept_feature`.
    ///
    /// Returning `false` does not prevent the `to` package from being included if it's reachable
    /// through other means.
    fn accept_package(&mut self, phase: CargoResolvePhase<'g, '_>, link: PackageLink<'g>) -> bool;

    /// Returns true if this feature link should be considered during a resolve operation.
    ///
    /// This is called for all resolvers. The default implementation forwards to `accept_package`.
    ///
    /// Returning `false` does not prevent the `to` package from being included if it's reachable
    /// through other means.
    ///
    /// The provided implementation forwards to `accept_package`. It is possible to customize this
    /// if necessary.
    fn accept_feature(&mut self, phase: CargoResolvePhase<'g, '_>, link: CrossLink<'g>) -> bool {
        self.accept_package(phase, link.package_link())
    }
}

impl<'g, 'a, T> CargoPostfilter<'g> for &'a mut T
where
    T: CargoPostfilter<'g>,
{
    fn accept_package(&mut self, phase: CargoResolvePhase<'g, '_>, link: PackageLink<'g>) -> bool {
        (**self).accept_package(phase, link)
    }

    fn accept_feature(&mut self, phase: CargoResolvePhase<'g, '_>, link: CrossLink<'g>) -> bool {
        (**self).accept_feature(phase, link)
    }
}

impl<'g, 'a> CargoPostfilter<'g> for Box<dyn CargoPostfilter<'g> + 'a> {
    fn accept_package(&mut self, phase: CargoResolvePhase<'g, '_>, link: PackageLink<'g>) -> bool {
        (**self).accept_package(phase, link)
    }

    fn accept_feature(&mut self, phase: CargoResolvePhase<'g, '_>, link: CrossLink<'g>) -> bool {
        (**self).accept_feature(phase, link)
    }
}

impl<'g, 'a> CargoPostfilter<'g> for &'a mut dyn CargoPostfilter<'g> {
    fn accept_package(&mut self, phase: CargoResolvePhase<'g, '_>, link: PackageLink<'g>) -> bool {
        (**self).accept_package(phase, link)
    }

    fn accept_feature(&mut self, phase: CargoResolvePhase<'g, '_>, link: CrossLink<'g>) -> bool {
        (**self).accept_feature(phase, link)
    }
}

/// This default implementation accepts all packages and features passed in.
impl<'g> CargoPostfilter<'g> for () {
    fn accept_package(
        &mut self,
        _phase: CargoResolvePhase<'g, '_>,
        _link: PackageLink<'g>,
    ) -> bool {
        true
    }

    fn accept_feature(&mut self, _phase: CargoResolvePhase<'g, '_>, _link: CrossLink<'g>) -> bool {
        true
    }
}

/// A wrapper that converts a function to a `CargoPostfilter`.
#[derive(Clone, Debug)]
pub struct PostfilterFn<F>(F);

impl<'g, F> PostfilterFn<F>
where
    F: FnMut(CargoResolvePhase<'g, '_>, PackageLink<'g>) -> bool,
{
    /// Creates a new `PostfilterFn` by wrapping the provided function.
    pub fn new(f: F) -> Self {
        PostfilterFn(f)
    }
}

impl<'g, F> CargoPostfilter<'g> for PostfilterFn<F>
where
    F: FnMut(CargoResolvePhase<'g, '_>, PackageLink<'g>) -> bool,
{
    fn accept_package(&mut self, phase: CargoResolvePhase<'g, '_>, link: PackageLink<'g>) -> bool {
        (self.0)(phase, link)
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
    build_dep_edge_ixs: SortedSet<EdgeIndex<PackageIx>>,
}

impl<'g> CargoSet<'g> {
    /// Creates a new `CargoSet` based on the given query and options.
    ///
    /// This is also accessible through `FeatureQuery::resolve_cargo()`, and it may be more
    /// convenient to use that if the code is written in a "fluent" style.
    pub fn new<PF>(query: FeatureQuery<'g>, opts: &mut CargoOptions<'_, PF>) -> Result<Self, Error>
    where
        PF: CargoPostfilter<'g>,
    {
        let build_state = CargoSetBuildState::new(&query, opts)?;
        Ok(build_state.build(query, opts))
    }

    /// Creates a new `CargoIntermediateSet` based on the given query and options.
    ///
    /// This set contains an over-estimate of targets and features.
    ///
    /// Not part of the stable API, exposed for testing.
    #[doc(hidden)]
    pub fn new_intermediate<PF>(
        query: FeatureQuery<'g>,
        opts: &mut CargoOptions<'_, PF>,
    ) -> Result<CargoIntermediateSet<'g>, Error>
    where
        PF: CargoPostfilter<'g>,
    {
        let build_state = CargoSetBuildState::new(&query, opts)?;
        Ok(build_state.build_intermediate(query, opts))
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
    ///
    /// This includes all procedural macros, including those specified in the initial query.
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
        self.proc_macro_edge_ixs
            .iter()
            .map(move |edge_ix| package_graph.edge_ix_to_link(*edge_ix))
    }

    /// Returns `PackageLink` instances for build dependencies from target packages.
    ///
    /// ## Notes
    ///
    /// For each link, the `from` is built on the target while the `to` is built on the host.
    /// It is possible (though rare) that a build dependency is also included as a normal
    /// dependency, or as a dev dependency in which case it will also be built on the target.
    ///
    /// The returned iterators will not include build dependencies of host packages -- those are
    /// also built on the host.
    pub fn build_dep_links<'a>(
        &'a self,
    ) -> impl Iterator<Item = PackageLink<'g>> + ExactSizeIterator + 'a {
        let package_graph = self.target_features.graph().package_graph;
        self.build_dep_edge_ixs
            .iter()
            .map(move |edge_ix| package_graph.edge_ix_to_link(*edge_ix))
    }
}

struct CargoSetBuildState {
    omitted_packages: SortedSet<NodeIndex<PackageIx>>,
}

impl CargoSetBuildState {
    fn new<'g, 'a, PF>(
        query: &FeatureQuery<'g>,
        opts: &mut CargoOptions<'a, PF>,
    ) -> Result<Self, Error>
    where
        PF: CargoPostfilter<'g>,
    {
        if query.direction() == DependencyDirection::Reverse {
            return Err(Error::CargoSetError(
                "attempted to compute for reverse query".into(),
            ));
        }

        let omitted_packages: SortedSet<_> = query
            .graph()
            .package_graph
            .package_ixs(opts.omitted_packages.iter().copied())?;

        Ok(Self { omitted_packages })
    }

    fn build<'g, 'a, PF>(
        self,
        query: FeatureQuery<'g>,
        opts: &mut CargoOptions<'a, PF>,
    ) -> CargoSet<'g>
    where
        PF: CargoPostfilter<'g>,
    {
        match opts.version {
            CargoResolverVersion::V1 => self.new_v1(query, opts, false),
            CargoResolverVersion::V1Install => {
                let avoid_dev_deps = !opts.include_dev;
                self.new_v1(query, opts, avoid_dev_deps)
            }
            CargoResolverVersion::V2 => self.new_v2(query, opts),
        }
    }

    fn build_intermediate<'g, 'a, PF>(
        self,
        query: FeatureQuery<'g>,
        opts: &mut CargoOptions<'a, PF>,
    ) -> CargoIntermediateSet<'g>
    where
        PF: CargoPostfilter<'g>,
    {
        match opts.version {
            CargoResolverVersion::V1 => self.new_v1_intermediate(query, opts, false),
            CargoResolverVersion::V1Install => {
                let avoid_dev_deps = !opts.include_dev;
                self.new_v1_intermediate(query, opts, avoid_dev_deps)
            }
            CargoResolverVersion::V2 => self.new_v2_intermediate(query, opts),
        }
    }

    fn new_v1<'g, 'a, PF>(
        self,
        query: FeatureQuery<'g>,
        opts: &mut CargoOptions<'a, PF>,
        avoid_dev_deps: bool,
    ) -> CargoSet<'g>
    where
        PF: CargoPostfilter<'g>,
    {
        self.build_set(query, opts, |query, opts| {
            self.new_v1_intermediate(query, opts, avoid_dev_deps)
        })
    }

    fn new_v2<'g, 'a, PF>(
        self,
        query: FeatureQuery<'g>,
        opts: &mut CargoOptions<'a, PF>,
    ) -> CargoSet<'g>
    where
        PF: CargoPostfilter<'g>,
    {
        self.build_set(query, opts, |query, opts| {
            self.new_v2_intermediate(query, opts)
        })
    }

    // ---
    // Helper methods
    // ---

    fn is_omitted(&self, package_ix: NodeIndex<PackageIx>) -> bool {
        self.omitted_packages.contains(&package_ix)
    }

    fn build_set<'g, 'a, PF>(
        &self,
        query: FeatureQuery<'g>,
        opts: &mut CargoOptions<'a, PF>,
        intermediate_fn: impl FnOnce(
            FeatureQuery<'g>,
            &mut CargoOptions<'a, PF>,
        ) -> CargoIntermediateSet<'g>,
    ) -> CargoSet<'g>
    where
        PF: CargoPostfilter<'g>,
    {
        // Prepare a package query for step 2.
        let graph = *query.graph();
        // Note that currently, proc macros specified in initials are built on both the target and
        // the host.
        let mut host_ixs = Vec::new();
        let target_ixs: Vec<_> = query
            .params
            .initials()
            .iter()
            .filter_map(|feature_ix| {
                let metadata = graph.metadata_for_ix(*feature_ix);
                let package_ix = metadata.package_ix();
                if metadata.package().is_proc_macro() {
                    // Proc macros are built on the host.
                    host_ixs.push(package_ix);
                    if opts.proc_macros_on_target {
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
        let intermediate_set = intermediate_fn(query, opts);
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

            let consider_dev = opts.include_dev && query.starts_from(from.id()).expect("valid ID");
            // Build dependencies are only considered if there's a build script.
            let consider_build = from.has_build_script();

            let mut follow_target = is_enabled(
                target_set,
                &link,
                DependencyKind::Normal,
                opts.target_platform,
            ) || (consider_dev
                && is_enabled(
                    target_set,
                    &link,
                    DependencyKind::Development,
                    opts.target_platform,
                ));

            // Proc macros build on the host, so for normal/dev dependencies redirect it to the host
            // instead.
            let mut proc_macro_redirect = follow_target && to.is_proc_macro();

            // Build dependencies are evaluated against the host platform.
            let mut build_dep_redirect = consider_build
                && is_enabled(target_set, &link, DependencyKind::Build, opts.host_platform);

            // If the postfilter returns false, don't traverse this edge at all.
            let included = follow_target || proc_macro_redirect || build_dep_redirect;
            if included
                && !opts
                    .postfilter
                    .accept_package(CargoResolvePhase::TargetPackage(query), link)
            {
                follow_target = false;
                proc_macro_redirect = false;
                build_dep_redirect = false;
            }

            // Finally, process what needs to be done.
            if build_dep_redirect || proc_macro_redirect {
                host_ixs.push(to.package_ix());
            }
            if build_dep_redirect {
                build_dep_edge_ixs.push(link.edge_ix());
            }
            if proc_macro_redirect {
                proc_macro_edge_ixs.push(link.edge_ix());
                follow_target = false;
            }

            follow_target
        });

        // 3. Figure out what packages will be included on the host platform.
        let host_ixs = SortedSet::new(host_ixs);
        let host_packages = graph
            .package_graph
            .query_from_parts(host_ixs, DependencyDirection::Forward)
            .resolve_with_fn(|query, link| {
                let (from, to) = link.endpoints();
                if self.is_omitted(to.package_ix()) {
                    // Pretend that the omitted set doesn't exist.
                    return false;
                }
                let consider_build = from.has_build_script();

                // Only normal and build dependencies are considered, regardless of whether this is
                // an initial. (Dev-dependencies of initials would have been considered in step 2).
                let res = is_enabled(host_set, &link, DependencyKind::Normal, opts.host_platform)
                    || (consider_build
                        && is_enabled(host_set, &link, DependencyKind::Build, opts.host_platform));

                res && opts
                    .postfilter
                    .accept_package(CargoResolvePhase::HostPackage(query), link)
            });

        // Finally, the features are whatever packages were selected, intersected with whatever
        // features were selected.
        let target_features = graph
            .resolve_packages(&target_packages, all_filter())
            .intersection(target_set);
        let host_features = graph
            .resolve_packages(&host_packages, all_filter())
            .intersection(host_set);

        CargoSet {
            target_features,
            host_features,
            proc_macro_edge_ixs: SortedSet::new(proc_macro_edge_ixs),
            build_dep_edge_ixs: SortedSet::new(build_dep_edge_ixs),
        }
    }

    fn new_v1_intermediate<'g, 'a, PF>(
        &self,
        query: FeatureQuery<'g>,
        opts: &mut CargoOptions<'a, PF>,
        avoid_dev_deps: bool,
    ) -> CargoIntermediateSet<'g>
    where
        PF: CargoPostfilter<'g>,
    {
        // Perform a "complete" feature query. This will provide more packages than will be
        // included in the final build, but for each package it will have the correct feature set.
        let complete_set = query.resolve_with_fn(|query, link| {
            let res = if self.is_omitted(link.to().package_ix()) {
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
            };

            res && opts
                .postfilter
                .accept_feature(CargoResolvePhase::V1Unified(query), link)
        });

        CargoIntermediateSet::Unified(complete_set)
    }

    fn new_v2_intermediate<'g, 'a, PF>(
        &self,
        query: FeatureQuery<'g>,
        opts: &mut CargoOptions<'a, PF>,
    ) -> CargoIntermediateSet<'g>
    where
        PF: CargoPostfilter<'g>,
    {
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
        let target = query.resolve_with_fn(|query, link| {
            let (from, to) = link.endpoints();

            if self.is_omitted(to.package_ix()) {
                // Pretend that the omitted set doesn't exist.
                return false;
            }

            let consider_dev =
                opts.include_dev && query.starts_from(from.feature_id()).expect("valid ID");
            // This resolver doesn't check for whether this package has a build script.
            let mut follow_target = is_enabled(&link, DependencyKind::Normal, opts.target_platform)
                || (consider_dev
                    && is_enabled(&link, DependencyKind::Development, opts.target_platform));

            // Proc macros build on the host, so for normal/dev dependencies redirect it to the host
            // instead.
            let mut proc_macro_redirect = follow_target && to.package().is_proc_macro();

            // Build dependencies are evaluated against the host platform.
            let mut build_dep_redirect =
                is_enabled(&link, DependencyKind::Build, opts.host_platform);

            // If the postfilter returns false, don't traverse this edge at all.
            let included = follow_target || proc_macro_redirect || build_dep_redirect;
            if included
                && !opts
                    .postfilter
                    .accept_feature(CargoResolvePhase::TargetFeature(query), link)
            {
                follow_target = false;
                proc_macro_redirect = false;
                build_dep_redirect = false;
            }

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
            .resolve_with_fn(|query, link| {
                let (from, to) = link.endpoints();
                if self.is_omitted(to.package_ix()) {
                    // Pretend that the omitted set doesn't exist.
                    return false;
                }
                // During feature resolution, the v2 resolver doesn't check for whether this package
                // has a build script. It also unifies dev dependencies of initials, even on the
                // host platform.
                let consider_dev = opts.include_dev
                    && target_query
                        .starts_from(from.feature_id())
                        .expect("valid ID");

                let res = is_enabled(&link, DependencyKind::Normal, opts.host_platform)
                    || is_enabled(&link, DependencyKind::Build, opts.host_platform)
                    || (consider_dev
                        && is_enabled(&link, DependencyKind::Development, opts.host_platform));

                res && opts
                    .postfilter
                    .accept_feature(CargoResolvePhase::HostFeature(query), link)
            });

        CargoIntermediateSet::TargetHost { target, host }
    }
}

/// An intermediate set representing an overestimate of what packages are built, but an accurate
/// summary of what features are built given a particular package.
///
/// Not part of the stable API, exposed for cargo-compare.
#[doc(hidden)]
#[derive(Debug)]
pub enum CargoIntermediateSet<'g> {
    Unified(FeatureSet<'g>),
    TargetHost {
        target: FeatureSet<'g>,
        host: FeatureSet<'g>,
    },
}

impl<'g> CargoIntermediateSet<'g> {
    #[doc(hidden)]
    pub fn target_host_sets(&self) -> (&FeatureSet<'g>, &FeatureSet<'g>) {
        match self {
            CargoIntermediateSet::Unified(set) => (set, set),
            CargoIntermediateSet::TargetHost { target, host } => (target, host),
        }
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
