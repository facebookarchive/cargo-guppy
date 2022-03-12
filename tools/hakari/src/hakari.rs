// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{
    explain::HakariExplain,
    toml_name_map,
    toml_out::{write_toml, HakariOutputOptions},
    CargoTomlError, HakariCargoToml, TomlOutError,
};
use bimap::BiHashMap;
use debug_ignore::DebugIgnore;
use guppy::{
    errors::TargetSpecError,
    graph::{
        cargo::{BuildPlatform, CargoOptions, CargoResolverVersion, CargoSet, InitialsPlatform},
        feature::{FeatureId, FeatureLabel, FeatureSet, StandardFeatures},
        DependencyDirection, PackageGraph, PackageMetadata,
    },
    platform::{Platform, PlatformSpec, TargetFeatures},
    PackageId,
};
use rayon::prelude::*;
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt,
    sync::Arc,
};

/// Configures and constructs [`Hakari`](Hakari) instances.
///
/// This struct provides a number of options that determine how `Hakari` instances are generated.
#[derive(Clone, Debug)]
pub struct HakariBuilder<'g> {
    graph: DebugIgnore<&'g PackageGraph>,
    hakari_package: Option<PackageMetadata<'g>>,
    pub(crate) platforms: Vec<Arc<Platform>>,
    resolver: CargoResolverVersion,
    pub(crate) verify_mode: bool,
    pub(crate) traversal_excludes: HashSet<&'g PackageId>,
    final_excludes: HashSet<&'g PackageId>,
    pub(crate) registries: BiHashMap<String, String>,
    unify_target_host: UnifyTargetHost,
    output_single_feature: bool,
    pub(crate) dep_format_version: DepFormatVersion,
}

impl<'g> HakariBuilder<'g> {
    /// Creates a new `HakariBuilder` instance from a `PackageGraph`.
    ///
    /// The Hakari package itself is usually present in the workspace. If so, specify its
    /// package ID, otherwise pass in `None`.
    ///
    /// Returns an error if a Hakari package ID is specified but it isn't known to the graph, or
    /// isn't in the workspace.
    pub fn new(
        graph: &'g PackageGraph,
        hakari_id: Option<&PackageId>,
    ) -> Result<Self, guppy::Error> {
        let hakari_package = hakari_id
            .map(|package_id| {
                let package = graph.metadata(package_id)?;
                if !package.in_workspace() {
                    return Err(guppy::Error::UnknownWorkspaceName(
                        package.name().to_string(),
                    ));
                }
                Ok(package)
            })
            .transpose()?;

        Ok(Self {
            graph: DebugIgnore(graph),
            hakari_package,
            platforms: vec![],
            resolver: CargoResolverVersion::V2,
            verify_mode: false,
            traversal_excludes: HashSet::new(),
            final_excludes: HashSet::new(),
            registries: BiHashMap::new(),
            unify_target_host: UnifyTargetHost::default(),
            output_single_feature: false,
            dep_format_version: DepFormatVersion::default(),
        })
    }

    /// Returns the `PackageGraph` used to construct this `Hakari` instance.
    pub fn graph(&self) -> &'g PackageGraph {
        *self.graph
    }

    /// Returns the Hakari package, or `None` if it wasn't passed into [`new`](Self::new).
    pub fn hakari_package(&self) -> Option<&PackageMetadata<'g>> {
        self.hakari_package.as_ref()
    }

    /// Reads the existing TOML file for the Hakari package from disk, returning a
    /// `HakariCargoToml`.
    ///
    /// This can be used with [`Hakari::to_toml_string`](Hakari::to_toml_string) to manage the
    /// contents of the Hakari package's TOML file on disk.
    ///
    /// Returns an error if there was an issue reading the TOML file from disk, or `None` if
    /// this builder was created without a Hakari package.
    pub fn read_toml(&self) -> Option<Result<HakariCargoToml, CargoTomlError>> {
        let hakari_package = self.hakari_package()?;
        let workspace_path = hakari_package
            .source()
            .workspace_path()
            .expect("hakari_package is in workspace");
        Some(HakariCargoToml::new_relative(
            self.graph.workspace().root(),
            workspace_path,
        ))
    }

    /// Sets a list of platforms for `hakari` to use.
    ///
    /// By default, `hakari` unifies features that are always enabled across all platforms. If
    /// builds are commonly performed on a few platforms, `hakari` can output platform-specific
    /// instructions for those builds.
    ///
    /// This currently supports target triples only, without further customization around
    /// target features or flags. In the future, this may support `cfg()` expressions using
    /// an [SMT solver](https://en.wikipedia.org/wiki/Satisfiability_modulo_theories).
    ///
    /// Call `set_platforms` with an empty list to reset to default behavior.
    ///
    /// Returns an error if a platform wasn't known to [`target_spec`], the library `hakari` uses
    /// to resolve platforms.
    pub fn set_platforms(
        &mut self,
        platforms: impl IntoIterator<Item = impl Into<Cow<'static, str>>>,
    ) -> Result<&mut Self, TargetSpecError> {
        self.platforms = platforms
            .into_iter()
            .map(|s| Ok(Arc::new(Platform::new(s.into(), TargetFeatures::Unknown)?)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self)
    }

    /// Returns the platforms set through `set_platforms`, or an empty list if no platforms are
    /// set.
    pub fn platforms(&self) -> impl Iterator<Item = &str> + ExactSizeIterator + '_ {
        self.platforms.iter().map(|platform| platform.triple_str())
    }

    /// Sets the Cargo resolver version.
    ///
    /// By default, `HakariBuilder` uses [version 2](CargoResolverVersion::V2) of the Cargo
    /// resolver. For more about Cargo resolvers, see the documentation for
    /// [`CargoResolverVersion`](CargoResolverVersion).
    pub fn set_resolver(&mut self, resolver: CargoResolverVersion) -> &mut Self {
        self.resolver = resolver;
        self
    }

    /// Returns the current Cargo resolver version.
    pub fn resolver(&self) -> CargoResolverVersion {
        self.resolver
    }

    /// Pretends that the provided packages don't exist during graph traversals.
    ///
    /// Users may wish to not consider certain packages while figuring out the unified feature set.
    /// Setting this option prevents those packages from being considered.
    ///
    /// Practically, this means that:
    /// * If a workspace package is specified, Cargo build simulations for it will not be run.
    /// * If a third-party package is specified, it will not be present in the output, nor will
    ///   any transitive dependencies or features enabled by it that aren't enabled any other way.
    ///   In other words, any packages excluded during traversal are also [excluded from the final
    ///   output](Self::add_final_excludes).
    ///
    /// Returns an error if any package IDs specified aren't known to the graph.
    pub fn add_traversal_excludes<'b>(
        &mut self,
        excludes: impl IntoIterator<Item = &'b PackageId>,
    ) -> Result<&mut Self, guppy::Error> {
        let traversal_exclude: Vec<&'g PackageId> = excludes
            .into_iter()
            .map(|package_id| Ok(self.graph.metadata(package_id)?.id()))
            .collect::<Result<_, _>>()?;
        self.traversal_excludes.extend(traversal_exclude);
        Ok(self)
    }

    /// Returns the packages currently excluded during graph traversals.
    ///
    /// Also returns the Hakari package if specified. This is because the Hakari package is treated
    /// as excluded while performing unification.
    pub fn traversal_excludes<'b>(&'b self) -> impl Iterator<Item = &'g PackageId> + 'b {
        let excludes = self.make_traversal_excludes();
        excludes.iter()
    }

    /// Returns true if a package ID is currently excluded during traversal.
    ///
    /// Also returns true for the Hakari package if specified. This is because the Hakari package is
    /// treated as excluded by the algorithm.
    ///
    /// Returns an error if this package ID isn't known to the underlying graph.
    pub fn is_traversal_excluded(&self, package_id: &PackageId) -> Result<bool, guppy::Error> {
        self.graph.metadata(package_id)?;

        let excludes = self.make_traversal_excludes();
        Ok(excludes.is_excluded(package_id))
    }

    /// Adds packages to be removed from the final output.
    ///
    /// Unlike [`traversal_excludes`](Self::traversal_excludes), these packages are considered
    /// during traversals, but removed at the end.
    ///
    /// Returns an error if any package IDs specified aren't known to the graph.
    pub fn add_final_excludes<'b>(
        &mut self,
        excludes: impl IntoIterator<Item = &'b PackageId>,
    ) -> Result<&mut Self, guppy::Error> {
        let final_excludes: Vec<&'g PackageId> = excludes
            .into_iter()
            .map(|package_id| Ok(self.graph.metadata(package_id)?.id()))
            .collect::<Result<_, _>>()?;
        self.final_excludes.extend(final_excludes);
        Ok(self)
    }

    /// Returns the packages to be removed from the final output.
    pub fn final_excludes<'b>(&'b self) -> impl Iterator<Item = &'g PackageId> + 'b {
        self.final_excludes.iter().copied()
    }

    /// Returns true if a package ID is currently excluded from the final output.
    ///
    /// Returns an error if this package ID isn't known to the underlying graph.
    pub fn is_final_excluded(&self, package_id: &PackageId) -> Result<bool, guppy::Error> {
        self.graph.metadata(package_id)?;
        Ok(self.final_excludes.contains(package_id))
    }

    /// Returns true if a package ID is excluded from either the traversal or the final output.
    ///
    /// Also returns true for the Hakari package if specified. This is because the Hakari package is
    /// treated as excluded by the algorithm.
    ///
    /// Returns an error if this package ID isn't known to the underlying graph.
    #[inline]
    pub fn is_excluded(&self, package_id: &PackageId) -> Result<bool, guppy::Error> {
        Ok(self.is_traversal_excluded(package_id)? || self.is_final_excluded(package_id)?)
    }

    /// Add alternate registries by (name, URL) pairs.
    ///
    /// This is a temporary workaround until [Cargo issue #9052](https://github.com/rust-lang/cargo/issues/9052)
    /// is resolved.
    pub fn add_registries(
        &mut self,
        registries: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> &mut Self {
        self.registries.extend(
            registries
                .into_iter()
                .map(|(name, url)| (name.into(), url.into())),
        );
        self
    }

    /// Whether and how to unify feature sets across target and host platforms.
    ///
    /// This is an advanced feature that most users don't need to set. For more information about
    /// this option, see the documentation for [`UnifyTargetHost`](UnifyTargetHost).
    pub fn set_unify_target_host(&mut self, unify_target_host: UnifyTargetHost) -> &mut Self {
        self.unify_target_host = unify_target_host;
        self
    }

    /// Returns the current value of `unify_target_host`.
    pub fn unify_target_host(&self) -> UnifyTargetHost {
        self.unify_target_host
    }

    /// Whether to unify feature sets for all dependencies.
    ///
    /// By default, Hakari only produces output for dependencies that are built with more
    /// than one feature set. If set to true, Hakari will produce outputs for all dependencies,
    /// including those that don't need to be unified.
    ///
    /// This is rarely needed in production, and is most useful for testing and debugging scenarios.
    pub fn set_output_single_feature(&mut self, output_single_feature: bool) -> &mut Self {
        self.output_single_feature = output_single_feature;
        self
    }

    /// Returns the current value of `output_single_feature`.
    pub fn output_single_feature(&self) -> bool {
        self.output_single_feature
    }

    /// Version of `workspace-hack = ...` lines to output.
    ///
    /// For more, see the documentation for [`DepFormatVersion`](DepFormatVersion).
    pub fn set_dep_format_version(&mut self, dep_format_version: DepFormatVersion) -> &mut Self {
        self.dep_format_version = dep_format_version;
        self
    }

    /// Returns the current value of `dep_format_version`.
    pub fn dep_format_version(&self) -> DepFormatVersion {
        self.dep_format_version
    }

    /// Computes the `Hakari` for this builder.
    pub fn compute(self) -> Hakari<'g> {
        Hakari::build(self)
    }

    // ---
    // Helper methods
    // ---

    #[cfg(feature = "cli-support")]
    pub(crate) fn traversal_excludes_only<'b>(
        &'b self,
    ) -> impl Iterator<Item = &'g PackageId> + 'b {
        self.traversal_excludes.iter().copied()
    }

    fn make_traversal_excludes<'b>(&'b self) -> TraversalExcludes<'g, 'b> {
        let hakari_package = if self.verify_mode {
            None
        } else {
            self.hakari_package.map(|package| package.id())
        };

        TraversalExcludes {
            excludes: &self.traversal_excludes,
            hakari_package,
        }
    }

    fn make_features_only<'b>(&'b self) -> FeatureSet<'g> {
        if self.verify_mode {
            match &self.hakari_package {
                Some(package) => package.to_package_set(),
                None => self.graph.resolve_none(),
            }
            .to_feature_set(StandardFeatures::Default)
        } else {
            self.graph.feature_graph().resolve_none()
        }
    }
}

#[cfg(feature = "cli-support")]
mod summaries {
    use super::*;
    use crate::summaries::HakariBuilderSummary;
    use guppy::platform::TargetFeatures;

    impl<'g> HakariBuilder<'g> {
        /// Constructs a `HakariBuilder` from a `PackageGraph` and a serialized summary.
        ///
        /// Requires the `cli-support` feature to be enabled.
        ///
        /// Returns an error if the summary references a package that's not present, or if there was
        /// some other issue while creating a `HakariBuilder` from the summary.
        pub fn from_summary(
            graph: &'g PackageGraph,
            summary: &HakariBuilderSummary,
        ) -> Result<Self, guppy::Error> {
            let hakari_package = summary
                .hakari_package
                .as_ref()
                .map(|name| graph.workspace().member_by_name(name))
                .transpose()?;
            let platforms = summary
                .platforms
                .iter()
                .map(|triple_str| {
                    let platform = Platform::new(triple_str.clone(), TargetFeatures::Unknown)
                        .map_err(|err| {
                            guppy::Error::TargetSpecError(
                                "while resolving hakari config or summary".to_owned(),
                                err,
                            )
                        })?;
                    Ok(platform.into())
                })
                .collect::<Result<Vec<_>, _>>()?;

            let registries: BiHashMap<_, _> = summary
                .registries
                .iter()
                .map(|(name, url)| (name.clone(), url.clone()))
                .collect();

            let traversal_excludes = summary
                .traversal_excludes
                .to_package_set_registry(
                    graph,
                    |name| registries.get_by_left(name).map(|s| s.as_str()),
                    "resolving hakari traversal-excludes",
                )?
                .package_ids(DependencyDirection::Forward)
                .collect();
            let final_excludes = summary
                .final_excludes
                .to_package_set_registry(
                    graph,
                    |name| registries.get_by_left(name).map(|s| s.as_str()),
                    "resolving hakari final-excludes",
                )?
                .package_ids(DependencyDirection::Forward)
                .collect();

            Ok(Self {
                graph: DebugIgnore(graph),
                hakari_package,
                resolver: summary.resolver,
                verify_mode: false,
                unify_target_host: summary.unify_target_host,
                output_single_feature: summary.output_single_feature,
                dep_format_version: summary.dep_format_version,
                platforms,
                registries,
                traversal_excludes,
                final_excludes,
            })
        }
    }
}

/// Whether to unify feature sets for a given dependency across target and host platforms.
///
/// Consider a dependency that is built as both normally (on the target platform) and in a build
/// script or proc macro. The normal dependency is considered to be built on the *target platform*,
/// and is represented in the `[dependencies]` section in the generated `Cargo.toml`.
/// The build dependency is built on the *host platform*, represented in the `[build-dependencies]`
/// section.
///
/// Now consider that the target and host platforms need two different sets of features:
///
/// ```toml
/// ## feature set on target platform
/// [dependencies]
/// my-dep = { version = "1.0", features = ["a", "b"] }
///
/// ## feature set on host platform
/// [build-dependencies]
/// my-dep = { version = "1.0", features = ["b", "c"] }
/// ```
///
/// Should hakari unify the feature sets across the `[dependencies]` and `[build-dependencies]`
/// feature sets?
///
/// Call `HakariBuilder::set_unify_target_host` to configure this option.
#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "proptest1", derive(proptest_derive::Arbitrary))]
#[cfg_attr(feature = "cli-support", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "cli-support", serde(rename_all = "kebab-case"))]
#[non_exhaustive]
pub enum UnifyTargetHost {
    /// Perform no unification across the target and host feature sets.
    ///
    /// This is the most conservative option, but it means that some dependencies may be built with
    /// two different sets of features. In this mode, Hakari will likely be significantly less
    /// efficient.
    None,

    /// Automatically choose between the [`UnifyIfBoth`](Self::UnifyIfBoth) and the
    /// [`ReplicateTargetOnHost`](Self::ReplicateTargetOnHost) options:
    /// * If the workspace contains proc macros, or crates that are build dependencies of other
    ///   crates, choose the `ReplicateTargetAsHost` strategy.
    /// * Otherwise, choose the `UnifyIfBoth` strategy.
    ///
    /// This is the default behavior.
    Auto,

    /// Perform unification across target and host feature sets, but only if a dependency is built
    /// on both the target and the host.
    ///
    /// This is useful if cross-compilations are uncommon and one wishes to avoid the same package
    /// being built two different ways: once for the target and once for the host.
    UnifyIfBoth,

    /// Perform unification across target and host feature sets, and also replicate all target-only
    /// lines to the host.
    ///
    /// This is most useful if some workspace packages are proc macros or build dependencies
    /// used by other packages.
    ReplicateTargetOnHost,
}

/// The default for `UnifyTargetHost`: automatically choose unification strategy based on the
/// workspace.
impl Default for UnifyTargetHost {
    #[inline]
    fn default() -> Self {
        UnifyTargetHost::Auto
    }
}

/// Version of `workspace-hack = ...` lines in other `Cargo.toml` files to use.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "cli-support", derive(serde::Deserialize, serde::Serialize))]
#[non_exhaustive]
pub enum DepFormatVersion {
    /// `workspace-hack = { path = ...}`. (Note the lack of a trailing space.)
    ///
    /// This was used until `cargo hakari 0.9.6`.
    #[cfg_attr(feature = "cli-support", serde(rename = "1"))]
    V1,

    /// `workspace-hack = { version = "0.1", path = ... }`. This was introduced in
    /// `cargo hakari 0.9.8`.
    #[cfg_attr(feature = "cli-support", serde(rename = "2"))]
    V2,
}

impl Default for DepFormatVersion {
    fn default() -> Self {
        DepFormatVersion::V1
    }
}

/// A key representing a platform and host/target. Returned by `Hakari`.
#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OutputKey {
    /// The index of the build platform for this key, or `None` if the computation was done in a
    /// platform-independent manner.
    pub platform_idx: Option<usize>,

    /// The build platform: target or host.
    pub build_platform: BuildPlatform,
}

/// The result of a Hakari computation.
///
/// This contains all the data required to generate a workspace package.
///
/// Produced by [`HakariBuilder::compute`](HakariBuilder::compute).
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Hakari<'g> {
    pub(crate) builder: HakariBuilder<'g>,

    /// The map built by Hakari of dependencies that need to be unified.
    ///
    /// This map is used to construct the TOML output. Public access is provided in case some
    /// post-processing needs to be done.
    pub output_map: OutputMap<'g>,

    /// The complete map of dependency build results built by Hakari.
    ///
    /// This map is not used to generate the TOML output.
    pub computed_map: ComputedMap<'g>,
}

impl<'g> Hakari<'g> {
    /// Returns the `HakariBuilder` used to create this instance.
    pub fn builder(&self) -> &HakariBuilder<'g> {
        &self.builder
    }

    /// Reads the existing TOML file for the Hakari package from disk, returning a
    /// `HakariCargoToml`.
    ///
    /// This can be used with [`to_toml_string`](Self::to_toml_string) to manage the contents of
    /// the given TOML file on disk.
    ///
    /// Returns an error if there was an issue reading the TOML file from disk, or `None` if
    /// the builder's [`hakari_package`](HakariBuilder::hakari_package) is `None`.
    pub fn read_toml(&self) -> Option<Result<HakariCargoToml, CargoTomlError>> {
        self.builder.read_toml()
    }

    /// Writes `[dependencies]` and other `Cargo.toml` lines to the given `fmt::Write` instance.
    ///
    /// `&mut String` and `fmt::Formatter` both implement `fmt::Write`.
    pub fn write_toml(
        &self,
        options: &HakariOutputOptions,
        out: impl fmt::Write,
    ) -> Result<(), TomlOutError> {
        write_toml(&self.builder, &self.output_map, options, out)
    }

    /// Returns a map of dependency names as present in the workspace-hack's `Cargo.toml` to their
    /// corresponding [`PackageMetadata`].
    ///
    /// Packages which have one version are present as their original names, while packages with
    /// more than one version have a hash appended to them.
    pub fn toml_name_map(&self) -> HashMap<Cow<'g, str>, PackageMetadata<'g>> {
        toml_name_map(&self.output_map)
    }

    /// Returns a `HakariExplain`, which can be used to print out why a specific package is
    /// in the workspace-hack's `Cargo.toml`.
    ///
    /// Returns an error if the package ID was not found in the output.
    pub fn explain(
        &self,
        package_id: &'g PackageId,
    ) -> Result<HakariExplain<'g, '_>, guppy::Error> {
        HakariExplain::new(self, package_id)
    }

    /// A convenience method around `write_toml` that returns a new string with `Cargo.toml` lines.
    ///
    /// The returned string is guaranteed to be valid TOML, and can be provided to
    /// a [`HakariCargoToml`](crate::HakariCargoToml) obtained from [`read_toml`](Self::read_toml).
    pub fn to_toml_string(&self, options: &HakariOutputOptions) -> Result<String, TomlOutError> {
        let mut out = String::new();
        self.write_toml(options, &mut out)?;
        Ok(out)
    }

    // ---
    // Helper methods
    // ---

    fn build(builder: HakariBuilder<'g>) -> Self {
        let graph = *builder.graph;
        let mut computed_map_build = ComputedMapBuild::new(&builder);
        let platform_specs: Vec<_> = builder
            .platforms
            .iter()
            .map(|platform| PlatformSpec::Platform(platform.clone()))
            .collect();

        let unify_target_host = builder.unify_target_host.to_impl(graph);

        // Collect all the dependencies that need to be unified, by platform and build type.
        let mut map_build: OutputMapBuild<'g> = OutputMapBuild::new(graph);
        map_build.insert_all(
            computed_map_build.iter(),
            builder.output_single_feature,
            unify_target_host,
        );

        if !builder.output_single_feature {
            // Adding packages might cause different feature sets for some dependencies. Simulate
            // further builds with the given target and host features, and use that to add in any
            // extra features that need to be considered.
            loop {
                let mut add_extra = HashSet::new();
                for (output_key, features) in map_build.iter_feature_sets() {
                    let initials_platform = match output_key.build_platform {
                        BuildPlatform::Target => InitialsPlatform::Standard,
                        BuildPlatform::Host => InitialsPlatform::Host,
                    };

                    let mut cargo_opts = CargoOptions::new();
                    let platform_spec = match output_key.platform_idx {
                        Some(idx) => platform_specs[idx].clone(),
                        None => PlatformSpec::Always,
                    };
                    // Third-party dependencies are built without including dev.
                    cargo_opts
                        .set_include_dev(false)
                        .set_initials_platform(initials_platform)
                        .set_platform(platform_spec)
                        .set_resolver(builder.resolver)
                        .add_omitted_packages(computed_map_build.excludes.iter());
                    let cargo_set = features
                        .into_cargo_set(&cargo_opts)
                        .expect("into_cargo_set processed successfully");

                    // Check the features for the cargo set to see if any further dependencies were
                    // built with a different result and weren't included in the hakari map
                    // originally.
                    for &(build_platform, feature_set) in cargo_set.all_features().iter() {
                        for feature_list in
                            feature_set.packages_with_features(DependencyDirection::Forward)
                        {
                            let dep = feature_list.package();
                            let dep_id = dep.id();
                            let v_mut = computed_map_build
                                .get_mut(output_key.platform_idx, dep_id)
                                .expect("full value should be present");

                            // Is it already present in the output?
                            let new_key = OutputKey {
                                platform_idx: output_key.platform_idx,
                                build_platform,
                            };

                            if map_build.is_inserted(new_key, dep_id) {
                                continue;
                            }

                            let this_list: BTreeSet<_> = feature_list.named_features().collect();

                            let already_present = v_mut.contains(build_platform, &this_list);
                            if !already_present {
                                // The feature list added by this dependency is non-unique.
                                v_mut.mark_fixed_up(build_platform, this_list);
                                add_extra.insert((output_key.platform_idx, dep_id));
                            }
                        }
                    }
                }

                if add_extra.is_empty() {
                    break;
                }

                map_build.insert_all(
                    add_extra.iter().map(|&(platform_idx, dep_id)| {
                        let v = computed_map_build
                            .get(platform_idx, dep_id)
                            .expect("full value should be present");
                        (platform_idx, dep_id, v)
                    }),
                    builder.output_single_feature,
                    unify_target_host,
                );
            }
        }

        let computed_map = computed_map_build.computed_map;
        let output_map = map_build.finish(&builder.final_excludes);

        Self {
            builder,
            output_map,
            computed_map,
        }
    }
}

/// The map used by Hakari to generate output TOML.
///
/// This is a two-level `BTreeMap`, where:
/// * the top-level keys are [`OutputKey`](OutputKey) instances.
/// * the inner map is keyed by dependency [`PackageId`](PackageId) instances, and the values are
///   the corresponding [`PackageMetadata`](PackageMetadata) for this dependency, and the set of
///   features enabled for this package.
///
/// This is an alias for the type of [`Hakari::output_map`](Hakari::output_map).
pub type OutputMap<'g> =
    BTreeMap<OutputKey, BTreeMap<&'g PackageId, (PackageMetadata<'g>, BTreeSet<&'g str>)>>;

/// The map of all build results computed by Hakari.
///
/// The keys are the platform index and the dependency's package ID, and the values are
/// [`ComputedValue`](ComputedValue) instances that represent the different feature sets this
/// dependency is built with on both the host and target platforms.
///
/// The values that are most interesting are the ones where maps have two elements or more: they
/// indicate dependencies with features that need to be unified.
///
/// This is an alias for the type of [`Hakari::computed_map`](Hakari::computed_map).
pub type ComputedMap<'g> = BTreeMap<(Option<usize>, &'g PackageId), ComputedValue<'g>>;

/// The values of a [`ComputedMap`](ComputedMap).
///
/// This represents a pair of `ComputedInnerMap` instances: one for the target platform and one for
/// the host. For more about the values, see the documentation for
/// [`ComputedInnerMap`](ComputedInnerMap).
#[derive(Clone, Debug, Default)]
pub struct ComputedValue<'g> {
    /// The feature sets built on the target platform.
    pub target_inner: ComputedInnerMap<'g>,

    /// The feature sets built on the host platform.
    pub host_inner: ComputedInnerMap<'g>,
}

/// A target map or a host map in a [`ComputedValue`](ComputedValue).
///
/// * The keys are sets of feature names (or empty for no features).
/// * The values are [`ComputedInnerValue`] instances.
pub type ComputedInnerMap<'g> = BTreeMap<BTreeSet<&'g str>, ComputedInnerValue<'g>>;

/// The values of [`ComputedInnerMap`].
#[derive(Clone, Debug, Default)]
pub struct ComputedInnerValue<'g> {
    /// The workspace packages, selected features, and include dev that cause the key in
    /// `ComputedMap` to be built with the feature set that forms the key of `ComputedInnerMap`.
    /// They are not defined to be in any particular order.
    pub workspace_packages: Vec<(PackageMetadata<'g>, StandardFeatures, bool)>,

    /// Whether at least one post-computation fixup was performed with this feature set.
    pub fixed_up: bool,
}

impl<'g> ComputedInnerValue<'g> {
    fn extend(&mut self, other: ComputedInnerValue<'g>) {
        self.workspace_packages.extend(other.workspace_packages);
        self.fixed_up |= other.fixed_up;
    }

    #[inline]
    fn push(
        &mut self,
        package: PackageMetadata<'g>,
        features: StandardFeatures,
        include_dev: bool,
    ) {
        self.workspace_packages
            .push((package, features, include_dev));
    }
}

#[derive(Debug)]
struct TraversalExcludes<'g, 'b> {
    excludes: &'b HashSet<&'g PackageId>,
    hakari_package: Option<&'g PackageId>,
}

impl<'g, 'b> TraversalExcludes<'g, 'b> {
    fn iter(&self) -> impl Iterator<Item = &'g PackageId> + 'b {
        self.excludes.iter().copied().chain(self.hakari_package)
    }

    fn is_excluded(&self, package_id: &PackageId) -> bool {
        self.hakari_package == Some(package_id) || self.excludes.contains(package_id)
    }
}

/// Intermediate build state used by Hakari.
#[derive(Debug)]
struct ComputedMapBuild<'g, 'b> {
    excludes: TraversalExcludes<'g, 'b>,
    computed_map: ComputedMap<'g>,
}

impl<'g, 'b> ComputedMapBuild<'g, 'b> {
    fn new(builder: &'b HakariBuilder<'g>) -> Self {
        // This was just None or All for a bit under the theory that feature sets are additive only,
        // but unfortunately we cannot exploit this property because it doesn't account for the fact
        // that some dependencies might not be built *at all*, under certain feature combinations.
        //
        // That's also why we simulate builds with and without dev-only dependencies in all cases.
        //
        // For example, for:
        //
        // ```toml
        // [dependencies]
        // dep = { version = "1", optional = true }
        //
        // [dev-dependencies]
        // dep = { version = "1", optional = true, features = ["dev-feature"] }
        //
        // [features]
        // default = ["dep"]
        // extra = ["dep/extra", "dep/dev-feature"]
        // ```
        //
        // | feature set | include dev | dep status         |
        // | ----------- | ----------- | ------------------ |
        // | none        | no          | not built          |
        // | none        | yes         | not built          |
        // | default     | no          | no features        |
        // | default     | yes         | dev-feature        |
        // | all         | no          | extra, dev-feature |
        // | all         | yes         | extra, dev-feature |
        //
        // (And there's further complexity possible with transitive deps as well.)
        let features_include_dev = [
            (StandardFeatures::None, false),
            (StandardFeatures::None, true),
            (StandardFeatures::Default, false),
            (StandardFeatures::Default, true),
            (StandardFeatures::All, false),
            (StandardFeatures::All, true),
        ];

        // Features for the "always" platform spec.
        let always_features = features_include_dev
            .iter()
            .map(|&(features, include_dev)| (None, PlatformSpec::Always, features, include_dev));

        // Features for specified platforms.
        let specified_features =
            features_include_dev
                .iter()
                .flat_map(|&(features, include_dev)| {
                    builder
                        .platforms
                        .iter()
                        .enumerate()
                        .map(move |(idx, platform)| {
                            (
                                Some(idx),
                                PlatformSpec::Platform(platform.clone()),
                                features,
                                include_dev,
                            )
                        })
                });
        let platforms_features: Vec<_> = always_features.chain(specified_features).collect();

        let workspace = builder.graph.workspace();
        let excludes = builder.make_traversal_excludes();
        let features_only = builder.make_features_only();
        let excludes_ref = &excludes;
        let features_only_ref = &features_only;

        let computed_map: ComputedMap<'g> = platforms_features
            .into_par_iter()
            // The cargo_set computation in the inner iterator is the most expensive part of the
            // process, so use flat_map instead of flat_map_iter.
            .flat_map(|(idx, platform_spec, feature_filter, include_dev)| {
                let mut cargo_options = CargoOptions::new();
                cargo_options
                    .set_include_dev(include_dev)
                    .set_resolver(builder.resolver)
                    .set_platform(platform_spec)
                    .add_omitted_packages(excludes.iter());

                workspace.par_iter().map(move |workspace_package| {
                    if excludes_ref.is_excluded(workspace_package.id()) {
                        // Skip this package since it was excluded during traversal.
                        return BTreeMap::new();
                    }

                    let initials = workspace_package
                        .to_package_set()
                        .to_feature_set(feature_filter);
                    let cargo_set =
                        CargoSet::new(initials, features_only_ref.clone(), &cargo_options)
                            .expect("cargo resolution should succeed");

                    let all_features = cargo_set.all_features();

                    let values = all_features.iter().flat_map(|&(build_platform, features)| {
                        features
                            .packages_with_features(DependencyDirection::Forward)
                            .filter_map(move |feature_list| {
                                let dep = feature_list.package();
                                if dep.in_workspace() {
                                    // Only looking at third-party packages for hakari.
                                    return None;
                                }

                                let features: BTreeSet<&'g str> =
                                    feature_list.named_features().collect();
                                Some((
                                    idx,
                                    build_platform,
                                    dep.id(),
                                    features,
                                    workspace_package,
                                    feature_filter,
                                    include_dev,
                                ))
                            })
                    });

                    let mut map = ComputedMap::new();
                    for (
                        platform_idx,
                        build_platform,
                        package_id,
                        features,
                        package,
                        feature_filter,
                        include_dev,
                    ) in values
                    {
                        // Accumulate the features and package for each key.
                        map.entry((platform_idx, package_id)).or_default().insert(
                            build_platform,
                            features,
                            package,
                            feature_filter,
                            include_dev,
                        );
                    }

                    map
                })
            })
            .reduce(ComputedMap::new, |mut acc, map| {
                // Accumulate across all threads.
                for (k, v) in map {
                    acc.entry(k).or_default().merge(v);
                }
                acc
            });

        Self {
            excludes,
            computed_map,
        }
    }

    fn get(
        &self,
        platform_idx: Option<usize>,
        package_id: &'g PackageId,
    ) -> Option<&ComputedValue<'g>> {
        self.computed_map.get(&(platform_idx, package_id))
    }

    fn get_mut(
        &mut self,
        platform_idx: Option<usize>,
        package_id: &'g PackageId,
    ) -> Option<&mut ComputedValue<'g>> {
        self.computed_map.get_mut(&(platform_idx, package_id))
    }

    fn iter<'a>(
        &'a self,
    ) -> impl Iterator<Item = (Option<usize>, &'g PackageId, &'a ComputedValue<'g>)> + 'a {
        self.computed_map
            .iter()
            .map(move |(&(platform_idx, package_id), v)| (platform_idx, package_id, v))
    }
}

impl<'g> ComputedValue<'g> {
    /// Returns both the inner maps along with the build platforms they represent.
    pub fn inner_maps(&self) -> [(BuildPlatform, &ComputedInnerMap<'g>); 2] {
        [
            (BuildPlatform::Target, &self.target_inner),
            (BuildPlatform::Host, &self.host_inner),
        ]
    }

    /// Converts `self` into [`ComputedInnerMap`] instances, along with the build platforms they
    /// represent.
    pub fn into_inner_maps(self) -> [(BuildPlatform, ComputedInnerMap<'g>); 2] {
        [
            (BuildPlatform::Target, self.target_inner),
            (BuildPlatform::Host, self.host_inner),
        ]
    }

    /// Returns a reference to the inner map corresponding to the given build platform.
    pub fn get_inner(&self, build_platform: BuildPlatform) -> &ComputedInnerMap<'g> {
        match build_platform {
            BuildPlatform::Target => &self.target_inner,
            BuildPlatform::Host => &self.host_inner,
        }
    }

    /// Returns a mutable reference to the inner map corresponding to the given build platform.
    pub fn get_inner_mut(&mut self, build_platform: BuildPlatform) -> &mut ComputedInnerMap<'g> {
        match build_platform {
            BuildPlatform::Target => &mut self.target_inner,
            BuildPlatform::Host => &mut self.host_inner,
        }
    }

    /// Adds all the instances in `other` to `self`.
    fn merge(&mut self, other: ComputedValue<'g>) {
        for (features, details) in other.target_inner {
            self.target_inner
                .entry(features)
                .or_default()
                .extend(details);
        }
        for (features, details) in other.host_inner {
            self.host_inner.entry(features).or_default().extend(details);
        }
    }

    fn contains(&mut self, build_platform: BuildPlatform, features: &BTreeSet<&'g str>) -> bool {
        self.get_inner(build_platform).contains_key(features)
    }

    fn insert(
        &mut self,
        build_platform: BuildPlatform,
        features: BTreeSet<&'g str>,
        package: PackageMetadata<'g>,
        feature_filter: StandardFeatures,
        include_dev: bool,
    ) {
        self.get_inner_mut(build_platform)
            .entry(features)
            .or_default()
            .push(package, feature_filter, include_dev);
    }

    fn mark_fixed_up(&mut self, build_platform: BuildPlatform, features: BTreeSet<&'g str>) {
        self.get_inner_mut(build_platform)
            .entry(features)
            .or_default()
            .fixed_up = true;
    }

    fn describe<'a>(&'a self) -> ValueDescribe<'g, 'a> {
        match (self.target_inner.len(), self.host_inner.len()) {
            (0, 0) => ValueDescribe::None,
            (0, 1) => ValueDescribe::SingleHost(&self.host_inner),
            (1, 0) => ValueDescribe::SingleTarget(&self.target_inner),
            (1, 1) => {
                let target_features = self.target_inner.keys().next().expect("1 element");
                let host_features = self.host_inner.keys().next().expect("1 element");
                if target_features == host_features {
                    ValueDescribe::SingleMatchingBoth {
                        target_inner: &self.target_inner,
                        host_inner: &self.host_inner,
                    }
                } else {
                    ValueDescribe::SingleNonMatchingBoth {
                        target_inner: &self.target_inner,
                        host_inner: &self.host_inner,
                    }
                }
            }
            (_m, 0) => ValueDescribe::MultiTarget(&self.target_inner),
            (_m, 1) => ValueDescribe::MultiTargetSingleHost {
                target_inner: &self.target_inner,
                host_inner: &self.host_inner,
            },
            (0, _n) => ValueDescribe::MultiHost(&self.host_inner),
            (1, _n) => ValueDescribe::MultiHostSingleTarget {
                target_inner: &self.target_inner,
                host_inner: &self.host_inner,
            },
            (_m, _n) => ValueDescribe::MultiBoth {
                target_inner: &self.target_inner,
                host_inner: &self.host_inner,
            },
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum ValueDescribe<'g, 'a> {
    None,
    SingleTarget(&'a ComputedInnerMap<'g>),
    SingleHost(&'a ComputedInnerMap<'g>),
    MultiTarget(&'a ComputedInnerMap<'g>),
    MultiHost(&'a ComputedInnerMap<'g>),
    SingleMatchingBoth {
        target_inner: &'a ComputedInnerMap<'g>,
        host_inner: &'a ComputedInnerMap<'g>,
    },
    SingleNonMatchingBoth {
        target_inner: &'a ComputedInnerMap<'g>,
        host_inner: &'a ComputedInnerMap<'g>,
    },
    MultiTargetSingleHost {
        target_inner: &'a ComputedInnerMap<'g>,
        host_inner: &'a ComputedInnerMap<'g>,
    },
    MultiHostSingleTarget {
        target_inner: &'a ComputedInnerMap<'g>,
        host_inner: &'a ComputedInnerMap<'g>,
    },
    MultiBoth {
        target_inner: &'a ComputedInnerMap<'g>,
        host_inner: &'a ComputedInnerMap<'g>,
    },
}

impl<'g, 'a> ValueDescribe<'g, 'a> {
    #[allow(dead_code)]
    fn description(self) -> &'static str {
        match self {
            ValueDescribe::None => "None",
            ValueDescribe::SingleTarget(_) => "SingleTarget",
            ValueDescribe::SingleHost(_) => "SingleHost",
            ValueDescribe::MultiTarget(_) => "MultiTarget",
            ValueDescribe::MultiHost(_) => "MultiHost",
            ValueDescribe::SingleMatchingBoth { .. } => "SingleMatchingBoth",
            ValueDescribe::SingleNonMatchingBoth { .. } => "SingleNonMatchingBoth",
            ValueDescribe::MultiTargetSingleHost { .. } => "MultiTargetSingleHost",
            ValueDescribe::MultiHostSingleTarget { .. } => "MultiHostSingleTarget",
            ValueDescribe::MultiBoth { .. } => "MultiBoth",
        }
    }

    fn insert(
        self,
        output_single_feature: bool,
        unify_target_host: UnifyTargetHostImpl,
        mut insert_cb: impl FnMut(BuildPlatform, &'a ComputedInnerMap<'g>),
    ) {
        use BuildPlatform::*;

        match self {
            ValueDescribe::None => {
                // Empty, ignore. (This should probably never happen anyway.)
            }
            ValueDescribe::SingleTarget(target_inner) => {
                // Just one way to unify these.
                if output_single_feature {
                    insert_cb(Target, target_inner);
                    if unify_target_host == UnifyTargetHostImpl::ReplicateTargetOnHost {
                        insert_cb(Host, target_inner);
                    }
                }
            }
            ValueDescribe::SingleHost(host_inner) => {
                // Just one way to unify other.
                if output_single_feature {
                    insert_cb(Host, host_inner);
                }
            }
            ValueDescribe::MultiTarget(target_inner) => {
                // Unify features for target.
                insert_cb(Target, target_inner);
                if unify_target_host == UnifyTargetHostImpl::ReplicateTargetOnHost {
                    insert_cb(Host, target_inner);
                }
            }
            ValueDescribe::MultiHost(host_inner) => {
                // Unify features for host.
                insert_cb(Host, host_inner);
            }
            ValueDescribe::SingleMatchingBoth {
                target_inner,
                host_inner,
            } => {
                // Just one way to unify across both.
                if output_single_feature {
                    insert_cb(Target, target_inner);
                    insert_cb(Host, host_inner);
                }
            }
            ValueDescribe::SingleNonMatchingBoth {
                target_inner,
                host_inner,
            } => {
                // Unify features for both across both.
                insert_cb(Target, target_inner);
                insert_cb(Host, host_inner);
                if unify_target_host != UnifyTargetHostImpl::None {
                    insert_cb(Target, host_inner);
                    insert_cb(Host, target_inner);
                }
            }
            ValueDescribe::MultiTargetSingleHost {
                target_inner,
                host_inner,
            } => {
                // Unify features for both across both.
                insert_cb(Target, target_inner);
                insert_cb(Host, host_inner);
                if unify_target_host != UnifyTargetHostImpl::None {
                    insert_cb(Target, host_inner);
                    insert_cb(Host, target_inner);
                }
            }
            ValueDescribe::MultiHostSingleTarget {
                target_inner,
                host_inner,
            } => {
                // Unify features for both across both.
                insert_cb(Target, target_inner);
                insert_cb(Host, host_inner);
                if unify_target_host != UnifyTargetHostImpl::None {
                    insert_cb(Target, host_inner);
                    insert_cb(Host, target_inner);
                }
            }
            ValueDescribe::MultiBoth {
                target_inner,
                host_inner,
            } => {
                // Unify features for both across both.
                insert_cb(Target, target_inner);
                insert_cb(Host, host_inner);
                if unify_target_host != UnifyTargetHostImpl::None {
                    insert_cb(Target, host_inner);
                    insert_cb(Host, target_inner);
                }
            }
        }
    }
}

#[derive(Debug)]
struct OutputMapBuild<'g> {
    graph: &'g PackageGraph,
    output_map: OutputMap<'g>,
}

impl<'g> OutputMapBuild<'g> {
    fn new(graph: &'g PackageGraph) -> Self {
        Self {
            graph,
            output_map: OutputMap::new(),
        }
    }

    fn is_inserted(&self, output_key: OutputKey, package_id: &'g PackageId) -> bool {
        match self.output_map.get(&output_key) {
            Some(inner_map) => inner_map.contains_key(package_id),
            None => false,
        }
    }

    #[allow(dead_code)]
    fn get(
        &self,
        output_key: OutputKey,
        package_id: &'g PackageId,
    ) -> Option<&(PackageMetadata<'g>, BTreeSet<&'g str>)> {
        match self.output_map.get(&output_key) {
            Some(inner_map) => inner_map.get(package_id),
            None => None,
        }
    }

    fn insert_all<'a>(
        &mut self,
        values: impl IntoIterator<Item = (Option<usize>, &'g PackageId, &'a ComputedValue<'g>)>,
        output_single_feature: bool,
        unify_target_host: UnifyTargetHostImpl,
    ) where
        'g: 'a,
    {
        for (platform_idx, dep_id, v) in values {
            let describe = v.describe();
            describe.insert(
                output_single_feature,
                unify_target_host,
                |build_platform, inner| {
                    self.insert_inner(platform_idx, build_platform, dep_id, inner);
                },
            );
        }
    }

    fn insert_inner(
        &mut self,
        platform_idx: Option<usize>,
        build_platform: BuildPlatform,
        package_id: &'g PackageId,
        inner: &ComputedInnerMap<'g>,
    ) {
        let output_key = OutputKey {
            platform_idx,
            build_platform,
        };
        self.insert(
            output_key,
            package_id,
            inner.keys().flat_map(|f| f.iter().copied()),
        )
    }

    fn insert(
        &mut self,
        output_key: OutputKey,
        package_id: &'g PackageId,
        features: impl IntoIterator<Item = &'g str>,
    ) {
        let map = self.output_map.entry(output_key).or_default();
        let graph = self.graph;
        let (_, inner) = map.entry(package_id).or_insert_with(|| {
            (
                graph.metadata(package_id).expect("valid package ID"),
                BTreeSet::new(),
            )
        });
        inner.extend(features);
    }

    fn iter_feature_sets<'a>(&'a self) -> impl Iterator<Item = (OutputKey, FeatureSet<'g>)> + 'a {
        self.output_map.iter().map(move |(&output_key, deps)| {
            let feature_ids = deps.iter().flat_map(|(&package_id, (_, features))| {
                features
                    .iter()
                    .map(move |&feature| FeatureId::new(package_id, FeatureLabel::Named(feature)))
            });
            (
                output_key,
                self.graph
                    .feature_graph()
                    .resolve_ids(feature_ids)
                    .expect("specified feature IDs are valid"),
            )
        })
    }

    fn finish(mut self, final_excludes: &HashSet<&'g PackageId>) -> OutputMap<'g> {
        // Remove all features that are already unified in the "always" set.
        for &build_platform in BuildPlatform::VALUES {
            let always_key = OutputKey {
                platform_idx: None,
                build_platform,
            };
            // Temporarily remove the set to avoid &mut issues.

            let always_map = match self.output_map.remove(&always_key) {
                Some(always_map) => always_map,
                None => {
                    // No packages unified for the always set.
                    continue;
                }
            };

            for (key, inner_map) in &mut self.output_map {
                // Treat the host and target maps as separate.
                if key.build_platform != build_platform {
                    continue;
                }
                for (package_id, (_always_package, always_features)) in &always_map {
                    let (package, remaining_features) = {
                        let (package, features) = match inner_map.get(package_id) {
                            Some(v) => v,
                            None => {
                                // The package ID isn't present in the platform-specific map --
                                // nothing to be done.
                                continue;
                            }
                        };
                        (*package, features - always_features)
                    };
                    if remaining_features.is_empty() {
                        // No features left.
                        inner_map.remove(package_id);
                    } else {
                        inner_map.insert(package_id, (package, remaining_features));
                    }
                }
            }

            // Put always_map back into the output map.
            self.output_map.insert(always_key, always_map);
        }

        // Remove final-excludes, and get rid of any maps that are empty.
        self.output_map.retain(|_, inner_map| {
            for package_id in final_excludes {
                inner_map.remove(package_id);
            }
            !inner_map.is_empty()
        });

        self.output_map
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
enum UnifyTargetHostImpl {
    None,
    UnifyIfBoth,
    ReplicateTargetOnHost,
}

impl UnifyTargetHost {
    fn to_impl(self, graph: &PackageGraph) -> UnifyTargetHostImpl {
        match self {
            UnifyTargetHost::None => UnifyTargetHostImpl::None,
            UnifyTargetHost::UnifyIfBoth => UnifyTargetHostImpl::UnifyIfBoth,
            UnifyTargetHost::ReplicateTargetOnHost => UnifyTargetHostImpl::ReplicateTargetOnHost,
            UnifyTargetHost::Auto => {
                let workspace_set = graph.resolve_workspace();
                // Is any package a proc macro?
                if workspace_set
                    .packages(DependencyDirection::Forward)
                    .any(|package| package.is_proc_macro())
                {
                    return UnifyTargetHostImpl::ReplicateTargetOnHost;
                }

                // Is any package a build dependency of any other?
                if workspace_set
                    .links(DependencyDirection::Forward)
                    .any(|link| link.build().is_present())
                {
                    return UnifyTargetHostImpl::ReplicateTargetOnHost;
                }

                UnifyTargetHostImpl::UnifyIfBoth
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UnifyTargetHost;
    use fixtures::json::JsonFixture;

    #[test]
    fn unify_target_host_auto() {
        // Test that this "guppy" fixture (which does not have internal proc macros or build deps)
        // turns into "unify if both".
        let res = UnifyTargetHost::Auto.to_impl(JsonFixture::metadata_guppy_78cb7e8().graph());
        assert_eq!(
            res,
            UnifyTargetHostImpl::UnifyIfBoth,
            "no proc macros => unify if both"
        );

        // Test that this "libra" fixture (which has internal proc macros) turns into "replicate
        // target on host".
        let res = UnifyTargetHost::Auto.to_impl(JsonFixture::metadata_libra_9ffd93b().graph());
        assert_eq!(
            res,
            UnifyTargetHostImpl::ReplicateTargetOnHost,
            "proc macros => replicate target on host"
        );

        // Test that the "builddep" fixture (which has an internal build dependency) turns into
        // "replicate target on host".
        let res = UnifyTargetHost::Auto.to_impl(JsonFixture::metadata_builddep().graph());
        assert_eq!(
            res,
            UnifyTargetHostImpl::ReplicateTargetOnHost,
            "internal build deps => replicate target on host"
        );
    }
}
