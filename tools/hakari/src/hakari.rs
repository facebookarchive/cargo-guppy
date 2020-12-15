// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{
    toml_out::{write_toml, TomlOptions},
    CargoTomlError, HakariCargoToml, TomlOutError,
};
use guppy::{
    debug_ignore::DebugIgnore,
    graph::{
        cargo::{BuildPlatform, CargoOptions, CargoResolverVersion, CargoSet, InitialsPlatform},
        feature::{FeatureId, FeatureSet, StandardFeatures},
        DependencyDirection, PackageGraph, PackageMetadata,
    },
    PackageId, Platform,
};
use rayon::prelude::*;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt,
};

/// Configures and constructs [`Hakari`](Hakari) instances.
///
/// This struct provides a number of options that determine how `Hakari` instances are generated.
#[derive(Clone, Debug)]
pub struct HakariBuilder<'g, 'a> {
    graph: DebugIgnore<&'g PackageGraph>,
    hakari_package: Option<PackageMetadata<'g>>,
    platforms: Vec<Platform<'a>>,
    version: CargoResolverVersion,
    verify_mode: bool,
    omitted_packages: HashSet<&'g PackageId>,
    unify_target_host: UnifyTargetHost,
    unify_all: bool,
}

impl<'g, 'a> HakariBuilder<'g, 'a> {
    /// Creates a new `HakariBuilder` instance from a `PackageGraph`.
    ///
    /// The Hakari package itself is usually present in the workspace. If so, specify its
    /// package ID, otherwise pass in `None`. If specified, this package is marked as special: see
    /// the documentation for [`set_verify_mode`](Self::set_verify_mode) for more.
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
            version: CargoResolverVersion::V1,
            verify_mode: false,
            omitted_packages: HashSet::new(),
            unify_target_host: UnifyTargetHost::default(),
            unify_all: false,
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

    /// Sets a list of platforms for `hakari` to use.
    ///
    /// By default, `hakari` unifies features across all platforms. This may not always be desired,
    /// so it is possible to set a list of platforms. If the features for a particular dependency
    /// only need to be unified on some platforms, `hakari` will output platform-specific
    /// instructions.
    ///
    /// Call `set_platforms` with an empty list to reset to default behavior.
    pub fn set_platforms(
        &mut self,
        platforms: impl IntoIterator<Item = Platform<'a>>,
    ) -> &mut Self {
        self.platforms = platforms.into_iter().collect();
        self
    }

    /// Returns the platforms set through `set_platforms`, or an empty list if no platforms are
    /// set.
    pub fn platforms(&self) -> &[Platform<'a>] {
        &self.platforms
    }

    /// Sets the Cargo resolver version.
    ///
    /// By default, `HakariBuilder` uses [version 1](CargoResolverVersion::V1) of the Cargo
    /// resolver. For more about Cargo resolvers, see the documentation for
    /// [`CargoResolverVersion`](CargoResolverVersion).
    pub fn set_resolver_version(&mut self, version: CargoResolverVersion) -> &mut Self {
        self.version = version;
        self
    }

    /// Returns the current Cargo resolver version.
    pub fn resolver_version(&self) -> CargoResolverVersion {
        self.version
    }

    /// Adds packages to not consider while performing unification.
    ///
    /// Users may wish to not consider certain packages while figuring out the unified feature set.
    /// Setting this option prevents those packages from being considered.
    ///
    /// Practically, this means that:
    /// * If a workspace package is specified, Cargo build simulations for it will not be run.
    /// * If a third-party package is specified, it will not be present in the output, nor will
    ///   any features enabled by it that aren't enabled any other way.
    ///
    /// Returns an error if any package IDs specified aren't known to the graph.
    pub fn add_omitted_packages<'b>(
        &mut self,
        omitted_packages: impl IntoIterator<Item = &'b PackageId>,
    ) -> Result<&mut Self, guppy::Error> {
        let omitted_packages: Vec<&'g PackageId> = omitted_packages
            .into_iter()
            .map(|package_id| Ok(self.graph.metadata(package_id)?.id()))
            .collect::<Result<_, _>>()?;
        self.omitted_packages.extend(omitted_packages);
        Ok(self)
    }

    /// Returns the currently omitted packages.
    pub fn omitted_packages<'b>(
        &'b self,
    ) -> impl Iterator<Item = &'g PackageId> + ExactSizeIterator + 'b {
        self.omitted_packages.iter().copied()
    }

    /// If set to true, runs Hakari in verify mode.
    ///
    /// By default, Hakari runs in generate mode: the goal of this mode is to update an existing
    /// Hakari package's TOML. In this mode, the Hakari package is always omitted from
    /// consideration and added to the omitted packages.
    ///
    /// In verify mode, the goal is to ensure that Cargo builds actually produce a unique set of
    /// features. In this mode, instead of being omitted, the Hakari package is always *included*
    /// in feature resolution (default features), through the `features_only` argument to
    /// [`CargoSet::new`](CargoSet::new). If, in the result, [`hakari_map`](Hakari::hakari_map)
    /// is empty, then features were unified.
    ///
    /// Setting this to true has no effect if the Hakari package is not specified at construction
    /// time.
    pub fn set_verify_mode(&mut self, verify_mode: bool) -> &mut Self {
        self.verify_mode = verify_mode;
        self
    }

    /// Returns the current value of `verify_mode`.
    pub fn verify_mode(&self) -> bool {
        self.verify_mode
    }

    /// Whether to unify feature sets across target and host platforms.
    ///
    /// By default, `hakari` does not perform any unification across the target and host platforms.
    /// This means that if a dependency is a target (regular) dependency with one set of features,
    /// and a host (build) dependency with a different set of features, the two are treated
    /// separately.
    ///
    /// For more information about this option, see the documentation for
    /// [`UnifyTargetHost`](UnifyTargetHost).
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
    pub fn set_unify_all(&mut self, unify_all: bool) -> &mut Self {
        self.unify_all = unify_all;
        self
    }

    /// Returns the current value of `unify_all`.
    pub fn unify_all(&self) -> bool {
        self.unify_all
    }

    /// Computes the `Hakari` for this builder.
    pub fn compute(self) -> Hakari<'g, 'a> {
        Hakari::build(self)
    }

    // ---
    // Helper methods
    // ---

    fn make_omitted_features_only<'b>(&'b self) -> (HakariOmitted<'g, 'b>, FeatureSet<'g>) {
        let (hakari_package, features_only) = if self.verify_mode {
            let features_only = match &self.hakari_package {
                Some(package) => package.to_package_set(),
                None => self.graph.resolve_none(),
            }
            .to_feature_set(StandardFeatures::Default);
            (None, features_only)
        } else {
            (
                self.hakari_package.map(|package| package.id()),
                self.graph.feature_graph().resolve_none(),
            )
        };

        (
            HakariOmitted {
                omitted: &self.omitted_packages,
                hakari_package,
            },
            features_only,
        )
    }
}

impl<'g, 'a> PartialEq for HakariBuilder<'g, 'a> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.graph.0, other.graph.0)
            && self.hakari_package().map(|package| package.id())
                == other.hakari_package.map(|package| package.id())
            && self.platforms == other.platforms
            && self.version == other.version
            && self.verify_mode == other.verify_mode
            && self.omitted_packages == other.omitted_packages
            && self.unify_target_host == other.unify_target_host
            && self.unify_all == other.unify_all
    }
}

impl<'g, 'a> Eq for HakariBuilder<'g, 'a> {}

#[cfg(feature = "summaries")]
mod summaries {
    use super::*;
    use crate::summaries::HakariBuilderSummary;

    impl<'g> HakariBuilder<'g, 'static> {
        /// Constructs a `HakariBuilder` from a `PackageGraph` and a serialized summary.
        ///
        /// Requires the `summaries` feature to be enabled.
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
                .map(|platform| {
                    platform.to_platform().map_err(|err| {
                        guppy::Error::TargetSpecError("deserializing Hakari platforms".into(), err)
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let omitted_packages = summary
                .omitted_packages
                .iter()
                .map(|summary_id| Ok(graph.metadata_by_summary_id(&summary_id)?.id()))
                .collect::<Result<HashSet<_>, _>>()?;

            Ok(Self {
                graph: DebugIgnore(graph),
                hakari_package,
                version: summary.version,
                verify_mode: summary.verify_mode,
                unify_target_host: summary.unify_target_host,
                unify_all: summary.unify_all,
                platforms,
                omitted_packages,
            })
        }
    }
}

/// Whether to unify feature sets for a given dependency across target and host platforms.
///
/// Call `HakariBuilder::set_unify_target_host` to configure this option.
#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "proptest010", derive(proptest_derive::Arbitrary))]
#[cfg_attr(feature = "summaries", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "summaries", serde(rename_all = "kebab-case"))]
#[non_exhaustive]
pub enum UnifyTargetHost {
    /// Perform no unification across the target and host feature sets.
    ///
    /// This is the default behavior.
    None,

    /// Perform unification across target and host feature sets, but only if a dependency is built
    /// on both the target and the host.
    ///
    /// This is useful if cross-compilations are uncommon and one wishes to avoid the same package
    /// being built two different ways: once for the target and once for the host.
    UnifyOnBoth,

    /// Perform unification across target and host feature sets, and also replicate all target-only
    /// lines to the host.
    ///
    /// This is most useful if every package in the workspace depends on the Hakari package, and
    /// some of those packages are built on the host (e.g. proc macros or build dependencies).
    ReplicateTargetAsHost,
}

/// The default for `UnifyTargetHost`: perform no unification.
impl Default for UnifyTargetHost {
    fn default() -> Self {
        UnifyTargetHost::None
    }
}

/// A key representing a platform and host/target. Returned by `Hakari`.
#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HakariKey {
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
pub struct Hakari<'g, 'a> {
    /// The builder used to generate this `Hakari` instance.
    pub builder: HakariBuilder<'g, 'a>,

    /// The map built by Hakari of dependencies that need to be unified.
    ///
    /// This map is used to build the TOML output. Public access is provided in case some
    /// post-processing needs to be done.
    pub hakari_map: HakariMap<'g>,

    /// The complete map built by Hakari.
    ///
    /// This map is not used to generate the TOML output. However, public access is provided
    pub full_map: HakariFullMap<'g>,
}

impl<'g, 'a> Hakari<'g, 'a> {
    /// Reads the existing TOML file from disk, returning a `HakariCargoToml`.
    ///
    /// This can be used with [`to_toml_string`](Self::to_toml_string) to manage the contents of
    /// the given TOML file on disk.
    ///
    /// Returns an error if there was an issue reading the TOML file from disk, or `None` if
    /// the builder's [`hakari_package`](HakariBuilder::hakari_package) is `None`.
    pub fn read_toml(&self) -> Option<Result<HakariCargoToml, CargoTomlError>> {
        let hakari_package = self.builder.hakari_package()?;
        let workspace_path = hakari_package
            .source()
            .workspace_path()
            .expect("hakari_package is in workspace");
        Some(HakariCargoToml::new_relative(
            self.builder.graph.workspace().root(),
            workspace_path,
        ))
    }

    /// Writes `[dependencies]` and other `Cargo.toml` lines to the given `fmt::Write` instance.
    ///
    /// `&mut String` and `fmt::Formatter` both implement `fmt::Write`.
    pub fn write_toml(
        &self,
        options: &TomlOptions,
        out: impl fmt::Write,
    ) -> Result<(), TomlOutError> {
        write_toml(&self.builder, &self.hakari_map, options, out)
    }

    /// A convenience method around `write_toml` that returns a new string with `Cargo.toml` lines.
    ///
    /// The returned string is guaranteed to be valid TOML, and can be provided to
    /// a [`HakariCargoToml`](crate::HakariCargoToml) obtained from [`read_toml`](Self::read_toml).
    pub fn to_toml_string(&self, options: &TomlOptions) -> Result<String, TomlOutError> {
        let mut out = String::new();
        self.write_toml(options, &mut out)?;
        Ok(out)
    }

    // ---
    // Helper methods
    // ---

    fn build(builder: HakariBuilder<'g, 'a>) -> Self {
        let graph = *builder.graph;
        let full_map_build = HakariFullMapBuild::new(&builder);

        // Collect all the dependencies that need to be unified, by platform and build type.
        let mut map_build: HakariMapBuild<'g> = HakariMapBuild::new(graph);
        map_build.insert_all(
            full_map_build.iter(),
            builder.unify_all,
            builder.unify_target_host,
        );

        if !builder.unify_all {
            // Adding packages might cause different feature sets for some dependencies. Simulate
            // further builds with the given target and host features, and use that to add in any
            // extra features that need to be considered.
            loop {
                let mut add_extra = HashMap::new();
                for (hakari_key, features) in map_build.iter_feature_sets() {
                    let initials_platform = match hakari_key.build_platform {
                        BuildPlatform::Target => InitialsPlatform::Standard,
                        BuildPlatform::Host => InitialsPlatform::Host,
                    };

                    let mut cargo_opts = CargoOptions::new();
                    // Third-party dependencies are built without including dev.
                    cargo_opts
                        .set_include_dev(false)
                        .set_initials_platform(initials_platform)
                        .set_platform(
                            hakari_key
                                .platform_idx
                                .map(|platform_idx| &builder.platforms[platform_idx]),
                        )
                        .set_version(builder.version)
                        .add_omitted_packages(full_map_build.hakari_omitted.iter());
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
                            let v = full_map_build
                                .get(hakari_key.platform_idx, dep_id)
                                .expect("full value should be present");
                            let new_key = HakariKey {
                                platform_idx: hakari_key.platform_idx,
                                build_platform,
                            };

                            if map_build.is_inserted(new_key, dep_id) {
                                continue;
                            }

                            // Figure out what *would* be inserted for this key. Does it match?
                            let mut any_inserted = false;
                            let mut to_insert = BTreeSet::new();
                            v.describe().insert(
                                true,
                                builder.unify_target_host,
                                |insert_platform, inner_map| {
                                    if insert_platform == build_platform {
                                        any_inserted = true;
                                        to_insert.extend(
                                            inner_map.keys().flat_map(|f| f.iter().copied()),
                                        );
                                    }
                                },
                            );
                            if any_inserted
                                && feature_list.features()
                                    != to_insert.iter().copied().collect::<Vec<_>>()
                            {
                                // The feature list added by this dependency is non-unique.
                                add_extra.insert((hakari_key.platform_idx, dep_id), v);
                            }
                        }
                    }
                }

                if add_extra.is_empty() {
                    break;
                }

                map_build.insert_all(
                    add_extra
                        .iter()
                        .map(|(&(platform_idx, dep_id), &v)| (platform_idx, dep_id, v)),
                    // Force insert by setting unify_all to true.
                    true,
                    builder.unify_target_host,
                );
            }
        }

        let full_map = full_map_build.full_map;

        Self {
            builder,
            hakari_map: map_build.hakari_map,
            full_map,
        }
    }
}

/// The internal data structure used by Hakari.
///
/// This is a two-level `BTreeMap`, where:
/// * the top-level keys are [`HakariKey`](HakariKey) instances.
/// * the inner map is keyed by dependency [`PackageId`](PackageId) instances, and the values are
///   the corresponding [`PackageMetadata`](PackageMetadata) for this dependency, and the set of
///   features enabled for this package.
///
/// This is an alias for the type of [`Hakari::hakari_map`](Hakari::hakari_map).
pub type HakariMap<'g> =
    BTreeMap<HakariKey, BTreeMap<&'g PackageId, (PackageMetadata<'g>, BTreeSet<&'g str>)>>;

/// The full internal data structure used by Hakari.
///
/// The keys are the platform index and the dependency's package ID, and the values are
/// [`HakariFullValue`](HakariFullValue) instances that represent the different feature sets this
/// dependency is built with.
///
/// The values that are most interesting are the ones where the `HakariFullValue` value has two
/// elements or more: they indicate features that need to be unified.
///
/// This is an alias for the type of [`Hakari::full_map`](Hakari::full_map).
pub type HakariFullMap<'g> = BTreeMap<(Option<usize>, &'g PackageId), HakariFullValue<'g>>;

/// The values of a [`HakariFullMap`](HakariFullMap).
///
/// This represents a pair of `HakariFullInner` instances: one for the target and one for the host.
/// For more about the values, see the documentation for [`HakariFullInner`](HakariFullInner).
#[derive(Clone, Debug, Default)]
pub struct HakariFullValue<'g> {
    /// The feature sets built on the target platform.
    pub target_inner: HakariFullInner<'g>,

    /// The feature sets built on the host platform.
    pub host_inner: HakariFullInner<'g>,
}

/// The inner map that forms the values in [`HakariFullMap`](HakariFullMap).
///
/// * The keys are sets of feature names (or empty for no features).
/// * The inner values are the workspace packages + selected features that cause the dependency in
///   `HakariKey` to be built with the given feature set. They are not defined to be in any
///   particular order.
pub type HakariFullInner<'g> =
    BTreeMap<BTreeSet<&'g str>, Vec<(PackageMetadata<'g>, StandardFeatures)>>;

#[derive(Debug)]
struct HakariOmitted<'g, 'b> {
    omitted: &'b HashSet<&'g PackageId>,
    hakari_package: Option<&'g PackageId>,
}

impl<'g, 'b> HakariOmitted<'g, 'b> {
    fn iter(&self) -> impl Iterator<Item = &'g PackageId> + 'b {
        self.omitted.iter().copied().chain(self.hakari_package)
    }

    fn is_omitted(&self, package_id: &PackageId) -> bool {
        self.hakari_package == Some(package_id) || self.omitted.contains(package_id)
    }
}

/// Intermediate build state used by Hakari.
#[derive(Debug)]
struct HakariFullMapBuild<'g, 'b> {
    hakari_omitted: HakariOmitted<'g, 'b>,
    full_map: HakariFullMap<'g>,
}

impl<'g, 'b> HakariFullMapBuild<'g, 'b> {
    fn new(builder: &'b HakariBuilder<'g, '_>) -> Self {
        let platforms_features: Vec<_> = if builder.platforms.is_empty() {
            StandardFeatures::VALUES
                .iter()
                .map(|&features| (None, None, features))
                .collect()
        } else {
            StandardFeatures::VALUES
                .iter()
                .flat_map(|&features| {
                    builder
                        .platforms
                        .iter()
                        .enumerate()
                        .map(move |(idx, platform)| (Some(idx), Some(platform), features))
                })
                .collect()
        };

        let workspace = builder.graph.workspace();
        let (hakari_omitted, features_only) = builder.make_omitted_features_only();
        let hakari_omitted_ref = &hakari_omitted;
        let features_only_ref = &features_only;

        let full_map: HakariFullMap<'g> = platforms_features
            .into_par_iter()
            // The cargo_set computation in the inner iterator is the most expensive part of the
            // process, so use flat_map instead of flat_map_iter.
            .flat_map(|(platform_idx, platform, feature_filter)| {
                let mut cargo_options = CargoOptions::new();
                cargo_options
                    .set_include_dev(true)
                    .set_version(builder.version)
                    .set_platform(platform)
                    .add_omitted_packages(hakari_omitted.iter());

                workspace.par_iter().map(move |workspace_package| {
                    if hakari_omitted_ref.is_omitted(workspace_package.id()) {
                        // Skip this package since it was omitted.
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
                                    feature_list.features().iter().copied().collect();
                                Some((
                                    platform_idx,
                                    build_platform,
                                    dep.id(),
                                    features,
                                    workspace_package,
                                    feature_filter,
                                ))
                            })
                    });

                    let mut map = HakariFullMap::new();
                    for (
                        platform_idx,
                        build_platform,
                        package_id,
                        features,
                        package,
                        feature_filter,
                    ) in values
                    {
                        // Accumulate the features and package for each key.
                        map.entry((platform_idx, package_id)).or_default().insert(
                            build_platform,
                            features,
                            package,
                            feature_filter,
                        );
                    }

                    map
                })
            })
            .reduce(HakariFullMap::new, |mut acc, map| {
                // Accumulate across all threads.
                for (k, v) in map {
                    acc.entry(k).or_default().merge(v);
                }
                acc
            });

        Self {
            hakari_omitted,
            full_map,
        }
    }

    fn get(
        &self,
        platform_idx: Option<usize>,
        package_id: &'g PackageId,
    ) -> Option<&HakariFullValue<'g>> {
        self.full_map.get(&(platform_idx, package_id))
    }

    fn iter<'a>(
        &'a self,
    ) -> impl Iterator<Item = (Option<usize>, &'g PackageId, &'a HakariFullValue<'g>)> + 'a {
        self.full_map
            .iter()
            .map(move |(&(platform_idx, package_id), v)| (platform_idx, package_id, v))
    }
}

impl<'g> HakariFullValue<'g> {
    /// Returns both the inner maps along with the build platforms they represent.
    pub fn inner_maps(&self) -> [(BuildPlatform, &HakariFullInner<'g>); 2] {
        [
            (BuildPlatform::Target, &self.target_inner),
            (BuildPlatform::Host, &self.host_inner),
        ]
    }

    /// Returns a mutable reference to the inner map corresponding to the given build platform.
    pub fn get_inner_mut(&mut self, build_platform: BuildPlatform) -> &mut HakariFullInner<'g> {
        match build_platform {
            BuildPlatform::Target => &mut self.target_inner,
            BuildPlatform::Host => &mut self.host_inner,
        }
    }

    /// Adds all the instances in `other` to `self`.
    fn merge(&mut self, other: HakariFullValue<'g>) {
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

    fn insert(
        &mut self,
        build_platform: BuildPlatform,
        features: BTreeSet<&'g str>,
        package: PackageMetadata<'g>,
        feature_filter: StandardFeatures,
    ) {
        self.get_inner_mut(build_platform)
            .entry(features)
            .or_default()
            .push((package, feature_filter));
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
    SingleTarget(&'a HakariFullInner<'g>),
    SingleHost(&'a HakariFullInner<'g>),
    MultiTarget(&'a HakariFullInner<'g>),
    MultiHost(&'a HakariFullInner<'g>),
    SingleMatchingBoth {
        target_inner: &'a HakariFullInner<'g>,
        host_inner: &'a HakariFullInner<'g>,
    },
    SingleNonMatchingBoth {
        target_inner: &'a HakariFullInner<'g>,
        host_inner: &'a HakariFullInner<'g>,
    },
    MultiTargetSingleHost {
        target_inner: &'a HakariFullInner<'g>,
        host_inner: &'a HakariFullInner<'g>,
    },
    MultiHostSingleTarget {
        target_inner: &'a HakariFullInner<'g>,
        host_inner: &'a HakariFullInner<'g>,
    },
    MultiBoth {
        target_inner: &'a HakariFullInner<'g>,
        host_inner: &'a HakariFullInner<'g>,
    },
}

impl<'g, 'a> ValueDescribe<'g, 'a> {
    fn insert(
        self,
        unify_all: bool,
        unify_target_host: UnifyTargetHost,
        mut insert_cb: impl FnMut(BuildPlatform, &'a HakariFullInner<'g>),
    ) {
        use BuildPlatform::*;

        match self {
            ValueDescribe::None => {
                // Empty, ignore. (This should probably never happen anyway.)
            }
            ValueDescribe::SingleTarget(target_inner) => {
                // Just one way to unify these.
                if unify_all {
                    insert_cb(Target, target_inner);
                    if unify_target_host == UnifyTargetHost::ReplicateTargetAsHost {
                        insert_cb(Host, target_inner);
                    }
                }
            }
            ValueDescribe::SingleHost(host_inner) => {
                // Just one way to unify other.
                if unify_all {
                    insert_cb(Host, host_inner);
                }
            }
            ValueDescribe::MultiTarget(target_inner) => {
                // Unify features for target.
                insert_cb(Target, target_inner);
                if unify_target_host == UnifyTargetHost::ReplicateTargetAsHost {
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
                if unify_all {
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
                if unify_target_host != UnifyTargetHost::None {
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
                if unify_target_host != UnifyTargetHost::None {
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
                if unify_target_host != UnifyTargetHost::None {
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
                if unify_target_host != UnifyTargetHost::None {
                    insert_cb(Target, host_inner);
                    insert_cb(Host, target_inner);
                }
            }
        }
    }
}

#[derive(Debug)]
struct HakariMapBuild<'g> {
    graph: &'g PackageGraph,
    hakari_map: HakariMap<'g>,
}

impl<'g> HakariMapBuild<'g> {
    fn new(graph: &'g PackageGraph) -> Self {
        Self {
            graph,
            hakari_map: HakariMap::new(),
        }
    }

    fn is_inserted(&self, hakari_key: HakariKey, package_id: &'g PackageId) -> bool {
        match self.hakari_map.get(&hakari_key) {
            Some(inner_map) => inner_map.contains_key(package_id),
            None => false,
        }
    }

    fn insert_all<'a>(
        &mut self,
        values: impl IntoIterator<Item = (Option<usize>, &'g PackageId, &'a HakariFullValue<'g>)>,
        unify_all: bool,
        unify_target_host: UnifyTargetHost,
    ) where
        'g: 'a,
    {
        for (platform_idx, dep_id, v) in values {
            let describe = v.describe();
            describe.insert(unify_all, unify_target_host, |build_platform, inner| {
                self.insert_inner(platform_idx, build_platform, dep_id, inner);
            });
        }
    }

    fn insert_inner(
        &mut self,
        platform_idx: Option<usize>,
        build_platform: BuildPlatform,
        package_id: &'g PackageId,
        inner: &HakariFullInner<'g>,
    ) {
        let hakari_key = HakariKey {
            platform_idx,
            build_platform,
        };
        self.insert(
            hakari_key,
            package_id,
            inner.keys().flat_map(|f| f.iter().copied()),
        )
    }

    fn insert(
        &mut self,
        hakari_key: HakariKey,
        package_id: &'g PackageId,
        features: impl IntoIterator<Item = &'g str>,
    ) {
        let map = self.hakari_map.entry(hakari_key).or_default();
        let graph = self.graph;
        let (_, inner) = map.entry(package_id).or_insert_with(|| {
            (
                graph.metadata(package_id).expect("valid package ID"),
                BTreeSet::new(),
            )
        });
        inner.extend(features);
    }

    fn iter_feature_sets<'a>(&'a self) -> impl Iterator<Item = (HakariKey, FeatureSet<'g>)> + 'a {
        self.hakari_map.iter().map(move |(&hakari_key, deps)| {
            let feature_ids = deps.iter().flat_map(|(&package_id, (_, features))| {
                features
                    .iter()
                    .map(move |&feature| FeatureId::new(package_id, feature))
            });
            (
                hakari_key,
                self.graph
                    .feature_graph()
                    .resolve_ids(feature_ids)
                    .expect("specified feature IDs are valid"),
            )
        })
    }
}
