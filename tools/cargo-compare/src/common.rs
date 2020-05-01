// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::type_conversions::ToGuppy;
use crate::GlobalContext;
use anyhow::Result;
use cargo::core::compiler::{CompileKind, CompileTarget, RustcTargetData};
use cargo::core::resolver::features::FeaturesFor;
use cargo::core::resolver::{HasDevUnits, ResolveOpts};
use cargo::core::{enable_nightly_features, PackageIdSpec, Workspace};
use cargo::ops::resolve_ws_with_opts;
use cargo::Config;
use guppy::graph::cargo::{CargoOptions, CargoResolverVersion, CargoSet};
use guppy::graph::feature::FeatureSet;
use guppy::graph::{DependencyDirection, PackageGraph};
use guppy::{PackageId, Platform, TargetFeatures};
use guppy_cmdlib::{CargoMetadataOptions, PackagesAndFeatures};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use structopt::StructOpt;

/// Options that are common to Guppy and Cargo.
///
/// Guppy supports more options than Cargo. This describes the minimal set that both support.
#[derive(Debug, StructOpt)]
pub struct GuppyCargoCommon {
    #[structopt(flatten)]
    pub pf: PackagesAndFeatures,

    /// Include dev dependencies for initial packages
    #[structopt(long = "include-dev")]
    pub include_dev: bool,

    /// Use new feature resolver
    #[structopt(long = "v2")]
    pub v2: bool,

    /// Evaluate for the target triple (default: current platform)
    #[structopt(long = "target")]
    pub target_platform: Option<String>,

    #[structopt(flatten)]
    pub metadata_opts: CargoMetadataOptions,
}

impl GuppyCargoCommon {
    /// Resolves data for this query using Cargo.
    pub fn resolve_cargo(&self, ctx: &GlobalContext) -> Result<FeatureMap> {
        let config = self.cargo_make_config(ctx)?;
        let root_manifest = self.cargo_discover_root(&config)?;
        let workspace = self.cargo_make_workspace(&config, &root_manifest)?;

        let compile_kind = match &self.target_platform {
            Some(platform) => CompileKind::Target(CompileTarget::new(platform)?),
            None => CompileKind::Host,
        };
        let target_data = RustcTargetData::new(&workspace, &[compile_kind])?;

        let resolve_opts = ResolveOpts::new(
            self.include_dev,
            &self.pf.features,
            self.pf.all_features,
            !self.pf.no_default_features,
        );
        let packages = &self.pf.packages;
        let specs: Vec<_> = if packages.is_empty() {
            // Pass in the entire workspace.
            workspace
                .members()
                .map(|package| PackageIdSpec::from_package_id(package.package_id()))
                .collect()
        } else {
            packages
                .iter()
                .map(|spec| PackageIdSpec::parse(&spec))
                .collect::<Result<_>>()?
        };

        let ws_resolve = resolve_ws_with_opts(
            &workspace,
            &target_data,
            &[compile_kind],
            &resolve_opts,
            &specs,
            if self.include_dev {
                HasDevUnits::Yes
            } else {
                HasDevUnits::No
            },
        )?;

        let targeted_resolve = ws_resolve.targeted_resolve;
        let resolved_features = ws_resolve.resolved_features;

        let mut target_map = BTreeMap::new();
        let mut host_map = BTreeMap::new();
        for pkg_id in targeted_resolve.iter() {
            // Note that for the V1 resolver the maps are going to be identical, since
            // platform-specific filtering happens much later in the process.
            // Also, use activated_features_unverified since it's possible for a particular (package
            // ID, features for) combination to be missing.
            let target_features =
                resolved_features.activated_features_unverified(pkg_id, FeaturesFor::NormalOrDev);
            target_map.insert(pkg_id.to_guppy(), target_features.to_guppy());
            let host_features =
                resolved_features.activated_features_unverified(pkg_id, FeaturesFor::HostDep);
            host_map.insert(pkg_id.to_guppy(), host_features.to_guppy());
        }

        Ok(FeatureMap {
            target_map,
            host_map,
        })
    }

    /// Resolves data for this query using Guppy.
    pub fn resolve_guppy(&self, _ctx: &GlobalContext, graph: &PackageGraph) -> Result<FeatureMap> {
        let feature_query = self.pf.make_feature_query(graph)?;

        // Note that guppy is more flexible than cargo here -- with the v1 feature resolver, it can
        // evaluate dependencies one of three ways:
        // 1. include dev deps (cargo build --tests)
        // 2. avoid dev deps for both feature and package resolution (cargo install,
        //    -Zavoid-dev-deps)
        // 3. consider dev deps in feature resolution but not in final package resolution. This is
        //    what a default cargo build without building tests does, but there's no way to get that
        //    information from cargo's APIs since dev-only dependencies are filtered out during the
        //    compile phase.
        //
        // guppy can do all 3, but because of cargo's API limitations we restrict ourselves to 1
        // and 2 for now.
        let version = match (self.v2, self.include_dev) {
            (true, _) => CargoResolverVersion::V2,
            (false, true) => {
                // Case 1 above.
                CargoResolverVersion::V1
            }
            (false, false) => {
                // Case 2 above.
                CargoResolverVersion::V1Install
            }
        };
        let (target_platform, host_platform, merge_maps) = match version {
            CargoResolverVersion::V2 => (
                Some(self.make_target_platform()?),
                Some(self.guppy_current_platform()?),
                false,
            ),
            CargoResolverVersion::V1 | CargoResolverVersion::V1Install => {
                // Cargo's V1 resolver does platform-specific filtering after resolution. It also
                // merges the host and target maps.
                (None, None, true)
            }
            _ => panic!("unknown resolver version {:?}", version),
        };

        let cargo_opts = CargoOptions::new()
            .with_version(version)
            .with_dev_deps(self.include_dev)
            .with_target_platform(target_platform.as_ref())
            .with_host_platform(host_platform.as_ref());
        let cargo_set = feature_query.resolve_cargo(&cargo_opts)?;

        Ok(FeatureMap::from_guppy(&cargo_set, merge_maps))
    }

    /// Returns a `Platform` corresponding to the target platform.
    pub fn make_target_platform(&self) -> Result<Platform<'static>> {
        match &self.target_platform {
            Some(triple) => Platform::new(triple, TargetFeatures::Unknown)
                .ok_or_else(|| anyhow::anyhow!("unknown triple: {}", triple)),
            None => self.guppy_current_platform(),
        }
    }

    // ---
    // Helper methods
    // ---

    fn cargo_make_config(&self, _ctx: &GlobalContext) -> Result<Config> {
        // XXX This should use the home dir from ctx, but that appears to cause caching to break???
        // XXX Use default() for now, figure this out at some point.
        let mut config = Config::default()?;

        // Prevent cargo from accessing the network.
        let frozen = true;
        let locked = true;
        let offline = true;

        let unstable_flags: Vec<String> = if self.v2 {
            enable_nightly_features();
            vec!["features=all".into()]
        } else {
            vec![]
        };

        config.configure(
            2,
            false,
            None,
            frozen,
            locked,
            offline,
            &None,
            &unstable_flags,
            &[],
        )?;

        Ok(config)
    }

    fn cargo_discover_root(&self, config: &Config) -> Result<PathBuf> {
        let manifest_path = self.metadata_opts.abs_manifest_path()?;
        // Create a workspace to discover the root manifest.
        let workspace = Workspace::new(&manifest_path, config)?;

        let root_dir = workspace.root();
        Ok(root_dir.join("Cargo.toml"))
    }

    fn cargo_make_workspace<'cfg>(
        &self,
        config: &'cfg Config,
        root_manifest: &Path,
    ) -> Result<Workspace<'cfg>> {
        // Now create another workspace with the root that was found.
        Workspace::new(root_manifest, config)
    }

    fn guppy_current_platform(&self) -> Result<Platform<'static>> {
        Platform::current().ok_or_else(|| anyhow::anyhow!("unknown current platform"))
    }
}

#[derive(Clone, Debug)]
pub struct FeatureMap {
    pub target_map: BTreeMap<PackageId, BTreeSet<String>>,
    pub host_map: BTreeMap<PackageId, BTreeSet<String>>,
}

impl FeatureMap {
    fn from_guppy(cargo_set: &CargoSet<'_>, merge_maps: bool) -> Self {
        if merge_maps {
            let unified_set = cargo_set.target_features().union(cargo_set.host_features());
            let unified_map = Self::feature_set_to_map(&unified_set);
            Self {
                target_map: unified_map.clone(),
                host_map: unified_map,
            }
        } else {
            let target_map = Self::feature_set_to_map(cargo_set.target_features());
            let host_map = Self::feature_set_to_map(cargo_set.host_features());
            Self {
                target_map,
                host_map,
            }
        }
    }

    fn feature_set_to_map(feature_set: &FeatureSet<'_>) -> BTreeMap<PackageId, BTreeSet<String>> {
        feature_set
            .packages_with_features(DependencyDirection::Forward)
            .map(|feature_list| {
                let features = feature_list
                    .features()
                    .iter()
                    .copied()
                    .map(|feature| feature.to_string())
                    .collect();
                (feature_list.package().id().clone(), features)
            })
            .collect()
    }
}
