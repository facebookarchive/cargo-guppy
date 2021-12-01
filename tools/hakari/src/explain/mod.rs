// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Information about why a dependency is in the workspace-hack.
//!
//! [`HakariExplain`] instances are produced by [`Hakari::explain`]. The current API is limited
//! to displaying these instances if the `cli-support` feature is enabled.

#[cfg(feature = "cli-support")]
mod display;
mod simplify;

#[cfg(feature = "cli-support")]
pub use display::HakariExplainDisplay;

use crate::{explain::simplify::*, Hakari};
use guppy::{
    graph::{cargo::BuildPlatform, feature::StandardFeatures, PackageGraph, PackageMetadata},
    PackageId,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use target_spec::Platform;

/// The result of a Hakari explain query.
///
/// Generated by [`Hakari::explain`].
#[derive(Clone, Debug)]
pub struct HakariExplain<'g, 'a> {
    #[cfg_attr(not(feature = "cli-support"), allow(dead_code))]
    graph: &'g PackageGraph,
    metadata: PackageMetadata<'g>,
    #[cfg_attr(not(feature = "cli-support"), allow(dead_code))]
    platforms: &'a [Arc<Platform>],
    target_map: ExplainMap<'g, 'a>,
    host_map: ExplainMap<'g, 'a>,
}

type ExplainMap<'g, 'a> = BTreeMap<&'a BTreeSet<&'g str>, ExplainInner<'g>>;

#[derive(Clone, Debug, Default)]
struct ExplainInner<'g> {
    #[cfg_attr(not(feature = "cli-support"), allow(dead_code))]
    workspace_packages: BTreeMap<&'g PackageId, ExplainInnerValue<'g>>,
    #[cfg_attr(not(feature = "cli-support"), allow(dead_code))]
    fixup_platforms: Vec<Simple<Option<usize>>>,
}

#[derive(Clone, Debug)]
#[allow(clippy::type_complexity)]
struct ExplainInnerValue<'g> {
    #[cfg_attr(not(feature = "cli-support"), allow(dead_code))]
    metadata: PackageMetadata<'g>,
    #[cfg_attr(not(feature = "cli-support"), allow(dead_code))]
    sets: Vec<(
        Simple<bool>,
        Simple<StandardFeatures>,
        Simple<Option<usize>>,
    )>,
}

impl<'g, 'a> HakariExplain<'g, 'a> {
    pub(crate) fn new(hakari: &'a Hakari<'g>, dep_id: &PackageId) -> Result<Self, guppy::Error> {
        let graph = hakari.builder.graph();
        let metadata = hakari.builder.graph().metadata(dep_id)?;
        let intermediate = ExplainIntermediate::new(hakari, metadata.id())?;

        let target_map = Self::simplify_map(hakari, intermediate.target_map);
        let host_map = Self::simplify_map(hakari, intermediate.host_map);

        Ok(Self {
            graph,
            metadata,
            platforms: &hakari.builder.platforms,
            target_map,
            host_map,
        })
    }

    fn simplify_map(hakari: &'a Hakari<'g>, map: IntermediateMap<'g, 'a>) -> ExplainMap<'g, 'a> {
        const STANDARD_FEATURES_COUNT: usize = 3;
        const INCLUDE_DEV_COUNT: usize = 2;
        // +1 for the None case
        let platform_count = hakari.builder.platforms.len() + 1;

        map.into_iter()
            .map(|(features, inner)| {
                let workspace_packages = inner
                    .workspace_packages
                    .into_iter()
                    .map(|(package_id, IntermediateInnerValue { metadata, sets })| {
                        let sets = simplify3(
                            &sets,
                            (INCLUDE_DEV_COUNT, STANDARD_FEATURES_COUNT, platform_count),
                        );
                        (package_id, ExplainInnerValue { metadata, sets })
                    })
                    .collect();
                let fixup_platforms = simplify1(&inner.fixup_platforms, platform_count);
                (
                    features,
                    ExplainInner {
                        workspace_packages,
                        fixup_platforms,
                    },
                )
            })
            .collect()
    }

    /// Returns [`PackageMetadata`] for the dependency associated with this `HakariExplain`
    /// instance.
    pub fn dependency(&self) -> PackageMetadata<'g> {
        self.metadata
    }

    /// Returns a displayer for the output.
    #[cfg(feature = "cli-support")]
    pub fn display<'explain>(&'explain self) -> HakariExplainDisplay<'g, 'a, 'explain> {
        HakariExplainDisplay::new(self)
    }

    // Used by the display module.
    #[allow(dead_code)]
    fn explain_maps(&self) -> [(BuildPlatform, &ExplainMap<'g, 'a>); 2] {
        [
            (BuildPlatform::Target, &self.target_map),
            (BuildPlatform::Host, &self.host_map),
        ]
    }
}

/// Pre-simplification map.
#[derive(Debug)]
struct ExplainIntermediate<'g, 'a> {
    target_map: IntermediateMap<'g, 'a>,
    host_map: IntermediateMap<'g, 'a>,
}

type IntermediateMap<'g, 'a> = BTreeMap<&'a BTreeSet<&'g str>, IntermediateInner<'g>>;

#[derive(Debug, Default)]
struct IntermediateInner<'g> {
    workspace_packages: BTreeMap<&'g PackageId, IntermediateInnerValue<'g>>,
    fixup_platforms: BTreeSet<Option<usize>>,
}

#[derive(Debug)]
struct IntermediateInnerValue<'g> {
    metadata: PackageMetadata<'g>,
    sets: BTreeSet<(bool, StandardFeatures, Option<usize>)>,
}

impl<'g, 'a> ExplainIntermediate<'g, 'a> {
    fn new(hakari: &'a Hakari<'g>, dep_id: &'g PackageId) -> Result<Self, guppy::Error> {
        let mut target_map: IntermediateMap<'g, 'a> = BTreeMap::new();
        let mut host_map: IntermediateMap<'g, 'a> = BTreeMap::new();

        // Look at the computed map to figure out which packages are built.
        let platform_idxs =
            std::iter::once(None).chain((0..=hakari.builder.platforms.len()).map(Some));
        let map_keys = platform_idxs.map(|idx| (idx, dep_id));

        for (platform, package_id) in map_keys {
            let computed_value = match hakari.computed_map.get(&(platform, package_id)) {
                Some(v) => v,
                None => continue,
            };

            for (build_platform, inner_map) in computed_value.inner_maps() {
                let map = match build_platform {
                    BuildPlatform::Target => &mut target_map,
                    BuildPlatform::Host => &mut host_map,
                };

                for (features, inner_value) in inner_map {
                    for &(workspace_package, standard_features, include_dev) in
                        &inner_value.workspace_packages
                    {
                        map.entry(features)
                            .or_default()
                            .workspace_packages
                            .entry(workspace_package.id())
                            .or_insert_with(|| IntermediateInnerValue {
                                metadata: workspace_package,
                                sets: BTreeSet::new(),
                            })
                            .sets
                            .insert((include_dev, standard_features, platform));
                    }

                    if inner_value.fixed_up {
                        map.entry(features)
                            .or_default()
                            .fixup_platforms
                            .insert(platform);
                    }
                }
            }
        }

        if target_map.is_empty() && host_map.is_empty() {
            return Err(guppy::Error::UnknownPackageId(dep_id.clone()));
        }

        Ok(Self {
            target_map,
            host_map,
        })
    }
}
