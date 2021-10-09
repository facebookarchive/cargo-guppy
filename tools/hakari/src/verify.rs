// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Code related to ensuring that `hakari` works properly.
//!
//! # Verification algorithm
//!
//! By default, Hakari runs in "generate mode": the goal of this mode is to update an existing
//! Hakari package's TOML. In this mode, the Hakari package is always omitted from
//! consideration and added to the omitted packages.
//!
//! In verify mode, the goal is to ensure that Cargo builds actually produce a unique set of
//! features for every third-party dependency. In this mode, instead of being omitted, the Hakari package is always *included*
//! in feature resolution (with default features), through the `features_only` argument to
//! [`CargoSet::new`](guppy::graph::cargo::CargoSet::new). If, in the result, the
//! [`output_map`](crate::Hakari::output_map) is empty, then features were unified.

use crate::{
    hakari::{ComputedInnerMap, ComputedInnerValue},
    HakariBuilder,
};
use guppy::{
    graph::{cargo::BuildPlatform, feature::StandardFeatures},
    PackageId,
};
use std::fmt;

impl<'g> HakariBuilder<'g> {
    /// Verify that `hakari` worked properly.
    ///
    /// Returns `Ok(())` if only one version of every third-party dependency was built, or a list of
    /// errors if at least one third-party dependency had more than one version built.
    ///
    /// For more about how this works, see the documentation for the [`verify`](crate::verify)
    /// module.
    pub fn verify(mut self) -> Result<(), VerifyErrors<'g>> {
        self.verify_mode = true;
        let hakari = self.compute();
        let builder = hakari.builder;

        if hakari.output_map.is_empty() {
            Ok(())
        } else {
            let mut errors = vec![];

            for ((platform_idx, package_id), v) in hakari.computed_map {
                for (build_platform, inner_map) in v.into_inner_maps() {
                    if inner_map.len() > 1 {
                        let error = VerifyError {
                            package_id,
                            platform: platform_idx
                                .map(|idx| builder.platforms[idx].triple_str().to_owned()),
                            build_platform,
                            inner_map,
                        };
                        errors.push(error);
                    }
                }
            }
            Err(VerifyErrors { errors })
        }
    }
}

/// A list of errors returned by [`HakariBuilder::verify`].
///
/// Most users will want to use the `Display` impl to print out the list of errors.
///
/// For more about how verification works, see the documentation for the [`verify`](crate::verify)
/// module.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct VerifyErrors<'g> {
    /// The errors returned.
    pub errors: Vec<VerifyError<'g>>,
}

impl<'g> fmt::Display for VerifyErrors<'g> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for error in &self.errors {
            writeln!(f, "{}", error)?;
        }
        Ok(())
    }
}

/// A single instance of a third-party dependency that is built with more than one feature set.
///
/// Most users will want to use the `Display` impl to print out this error.
///
/// Forms part of [`VerifyErrors`], returned by [`HakariBuilder::verify`].
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct VerifyError<'g> {
    /// The third-party dependency built with more than one feature set.
    pub package_id: &'g PackageId,

    /// The platform the third-party dependency was built on.
    ///
    /// This is `None` if platforms were not specified.
    pub platform: Option<String>,

    /// The build platform (target or host).
    pub build_platform: BuildPlatform,

    /// Information about feature sets and the workspace packages that caused it.
    pub inner_map: ComputedInnerMap<'g>,
}

impl<'g> fmt::Display for VerifyError<'g> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.platform {
            Some(platform) => write!(f, "for platform {} ", platform)?,
            None => {}
        };
        writeln!(
            f,
            "on build platform {}, with third-party package {}:",
            self.build_platform, self.package_id
        )?;
        for (
            feature_set,
            ComputedInnerValue {
                workspace_packages,
                fixed_up,
            },
        ) in &self.inner_map
        {
            if feature_set.is_empty() {
                writeln!(f, "  for dependency with no features, workspace packages:")?;
            } else {
                let features: Vec<_> = feature_set.iter().copied().collect();
                writeln!(
                    f,
                    "  for dependency features [{}], workspace packages:",
                    features.join(", ")
                )?;
            }
            for (package, standard_features, include_dev) in workspace_packages {
                let feature_str = match standard_features {
                    StandardFeatures::None => "no features",
                    StandardFeatures::Default => "default features",
                    StandardFeatures::All => "all features",
                };
                let include_dev_str = match include_dev {
                    true => "including dev",
                    false => "excluding dev",
                };
                writeln!(
                    f,
                    "    * {} ({}, {})",
                    package.name(),
                    feature_str,
                    include_dev_str
                )?;
            }
            if *fixed_up {
                writeln!(f, "    * at least one post-compute fixup")?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::HakariBuilder;
    use guppy::MetadataCommand;

    /// Verify that this repo's `workspace-hack` works correctly.
    #[test]
    fn cargo_guppy_verify() {
        let graph = MetadataCommand::new()
            .build_graph()
            .expect("package graph built correctly");
        let workspace_hack = graph
            .workspace()
            .member_by_name("workspace-hack")
            .expect("this repo contains a workspace-hack package");
        let builder =
            HakariBuilder::new(&graph, Some(workspace_hack.id())).expect("builder initialized");
        if let Err(errs) = builder.verify() {
            panic!("verify failed: {}", errs);
        }
    }
}
