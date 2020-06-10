// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::cargo::build::CargoSetBuildState;
use crate::graph::feature::{CrossLink, FeatureQuery, FeatureSet};
use crate::graph::{PackageIx, PackageLink, PackageQuery};
use crate::sorted_set::SortedSet;
use crate::{Error, PackageId};
use petgraph::prelude::*;
use std::collections::HashSet;
use supercow::Supercow;
use target_spec::Platform;

/// Options for queries which simulate what Cargo does.
///
/// This provides control over the resolution algorithm used by `guppy`'s simulation of Cargo.
#[derive(Clone, Debug)]
pub struct CargoOptions<'a, PF = ()> {
    // Keep operations that can be used through a & reference separate, so that it can be shared
    // more easily.
    pub(super) imm_options: CargoImmOptions<'a>,
    pub(super) postfilter: PF,
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
            imm_options: CargoImmOptions::new(),
            postfilter,
        }
    }

    /// Sets the Cargo feature resolver version.
    ///
    /// For more about feature resolution, see the documentation for `CargoResolverVersion`.
    pub fn with_version(mut self, version: CargoResolverVersion) -> Self {
        self.imm_options.version = version;
        self
    }

    /// If set to true, causes dev-dependencies of the initial set to be followed.
    ///
    /// This does not affect transitive dependencies -- for example, a build or dev-dependency's
    /// further dev-dependencies are never followed.
    ///
    /// The default is true, which matches what a plain `cargo build` does.
    pub fn with_dev_deps(mut self, include_dev: bool) -> Self {
        self.imm_options.include_dev = include_dev;
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
        self.imm_options.proc_macros_on_target = proc_macros_on_target;
        self
    }

    /// Sets both the target and host platforms to the provided one, or to evaluate against any
    /// platform if `None`.
    pub fn with_platform(
        mut self,
        platform: Option<impl Into<Supercow<'a, Platform<'a>>>>,
    ) -> Self {
        let platform = Self::convert_platform(platform);
        self.imm_options.target_platform = platform.clone();
        self.imm_options.host_platform = platform;
        self
    }

    /// Sets the target platform to the provided one, or to evaluate against any platform if `None`.
    pub fn with_target_platform(
        mut self,
        target_platform: Option<impl Into<Supercow<'a, Platform<'a>>>>,
    ) -> Self {
        self.imm_options.target_platform = Self::convert_platform(target_platform);
        self
    }

    /// Sets the host platform to the provided one, or to evaluate against any platform if `None`.
    pub fn with_host_platform(
        mut self,
        host_platform: Option<impl Into<Supercow<'a, Platform<'a>>>>,
    ) -> Self {
        self.imm_options.host_platform = Self::convert_platform(host_platform);
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
        self.imm_options.omitted_packages.extend(package_ids);
        self
    }

    // ---
    // Helper methods
    // ---

    fn convert_platform(
        platform: Option<impl Into<Supercow<'a, Platform<'a>>>>,
    ) -> Option<Supercow<'a, Platform<'a>>> {
        platform.map(|platform| platform.into())
    }
}

impl<'a> Default for CargoOptions<'a> {
    fn default() -> Self {
        Self::new()
    }
}

/// Immutable options used by Cargo.
#[derive(Clone, Debug)]
pub(super) struct CargoImmOptions<'a> {
    pub(super) version: CargoResolverVersion,
    pub(super) include_dev: bool,
    pub(super) proc_macros_on_target: bool,
    // Use Supercow here to ensure that owned Platform instances are boxed, to reduce stack size.
    pub(super) host_platform: Option<Supercow<'a, Platform<'a>>>,
    pub(super) target_platform: Option<Supercow<'a, Platform<'a>>>,
    pub(super) omitted_packages: HashSet<&'a PackageId>,
}

impl<'a> CargoImmOptions<'a> {
    fn new() -> Self {
        Self {
            version: CargoResolverVersion::V1,
            include_dev: false,
            proc_macros_on_target: false,
            host_platform: None,
            target_platform: None,
            omitted_packages: HashSet::new(),
        }
    }

    pub(super) fn target_platform(&self) -> Option<&Platform<'a>> {
        self.target_platform.as_deref()
    }

    pub(super) fn host_platform(&self) -> Option<&Platform<'a>> {
        self.host_platform.as_deref()
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
    pub(super) target_features: FeatureSet<'g>,
    pub(super) host_features: FeatureSet<'g>,
    pub(super) proc_macro_edge_ixs: SortedSet<EdgeIndex<PackageIx>>,
    pub(super) build_dep_edge_ixs: SortedSet<EdgeIndex<PackageIx>>,
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
