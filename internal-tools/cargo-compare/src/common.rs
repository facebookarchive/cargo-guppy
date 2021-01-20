// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{type_conversions::ToGuppy, GlobalContext};
use anyhow::Result;
use cargo::{
    core::{
        compiler::{CompileKind, CompileTarget, RustcTargetData},
        enable_nightly_features,
        resolver::{
            features::{FeaturesFor, RequestedFeatures},
            ForceAllTargets, HasDevUnits, ResolveOpts,
        },
        PackageIdSpec, Workspace,
    },
    ops::resolve_ws_with_opts,
    util::interning::InternedString,
    Config,
};
use guppy::{
    graph::{
        cargo::{CargoOptions, CargoResolverVersion, CargoSet},
        feature::FeatureSet,
        DependencyDirection, PackageGraph,
    },
    PackageId, Platform, TargetFeatures,
};
use guppy_cmdlib::{proptest::triple_strategy, CargoMetadataOptions, PackagesAndFeatures};
use proptest::prelude::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    rc::Rc,
};
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
    pub fn resolve_cargo(&self, ctx: &GlobalContext<'_>) -> Result<FeatureMap> {
        let config = self.cargo_make_config(ctx)?;
        let root_manifest = self.cargo_discover_root(&config)?;
        let workspace = self.cargo_make_workspace(&config, &root_manifest)?;

        let compile_kind = match &self.target_platform {
            Some(platform) => CompileKind::Target(CompileTarget::new(platform)?),
            None => CompileKind::Host,
        };
        let target_data = RustcTargetData::new(&workspace, &[compile_kind])?;

        // TODO: last three arguments will become self.cargo_make_requested_features() in a future
        // Cargo release.
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
            // TODO: allow for target to be "any", set this to Yes in that case
            ForceAllTargets::No,
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
            if let Some(target_features) =
                resolved_features.activated_features_unverified(pkg_id, FeaturesFor::NormalOrDev)
            {
                target_map.insert(pkg_id.to_guppy(), target_features.to_guppy());
            }
            if let Some(host_features) =
                resolved_features.activated_features_unverified(pkg_id, FeaturesFor::HostDep)
            {
                host_map.insert(pkg_id.to_guppy(), host_features.to_guppy());
            }
        }

        Ok(FeatureMap {
            target_map,
            host_map,
        })
    }

    /// Resolves data for this query using Guppy.
    pub fn resolve_guppy(&self, ctx: &GlobalContext<'_>) -> Result<FeatureMap> {
        // Ignore the features-only set for now.
        // TODO: It would be interesting to test against it in the future.
        let (initials, _) = self.pf.make_feature_sets(ctx.graph())?;

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

        let target_platform = self.make_target_platform()?;
        let host_platform = self.guppy_current_platform()?;

        let mut cargo_opts = CargoOptions::new();
        cargo_opts
            .set_version(version)
            .set_include_dev(self.include_dev)
            .set_target_platform(Some(&target_platform))
            .set_host_platform(Some(&host_platform));
        let intermediate_set = CargoSet::new_intermediate(&initials, &cargo_opts)?;
        let (target_features, host_features) = intermediate_set.target_host_sets();

        Ok(FeatureMap::from_guppy(target_features, host_features))
    }

    /// Returns a `Platform` corresponding to the target platform.
    pub fn make_target_platform(&self) -> Result<Platform<'static>> {
        match &self.target_platform {
            Some(triple) => Ok(Platform::new(triple, TargetFeatures::Unknown)?),
            None => self.guppy_current_platform(),
        }
    }

    pub fn strategy<'a>(
        metadata_opts: &'a CargoMetadataOptions,
        graph: &'a PackageGraph,
    ) -> impl Strategy<Value = Self> + 'a {
        (
            PackagesAndFeatures::strategy(graph),
            any::<bool>(),
            any::<bool>(),
            triple_strategy(),
        )
            .prop_map(move |(pf, include_dev, v2, target_platform)| Self {
                pf,
                include_dev,
                v2,
                target_platform: target_platform.map(|s| s.to_string()),
                metadata_opts: metadata_opts.clone(),
            })
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

    #[allow(dead_code)]
    fn cargo_make_requested_features(&self) -> RequestedFeatures {
        let features: BTreeSet<_> = self
            .pf
            .features
            .iter()
            .map(|feature| InternedString::new(feature))
            .collect();
        RequestedFeatures {
            features: Rc::new(features),
            all_features: self.pf.all_features,
            uses_default_features: !self.pf.no_default_features,
        }
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
    fn from_guppy(target_features: &FeatureSet<'_>, host_features: &FeatureSet<'_>) -> Self {
        let target_map = Self::feature_set_to_map(target_features);
        let host_map = Self::feature_set_to_map(host_features);
        Self {
            target_map,
            host_map,
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
