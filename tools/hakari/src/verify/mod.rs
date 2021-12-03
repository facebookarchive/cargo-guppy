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

#[cfg(feature = "cli-support")]
mod display;

#[cfg(feature = "cli-support")]
pub use display::VerifyErrorsDisplay;

use crate::{explain::HakariExplain, Hakari, HakariBuilder};
use guppy::PackageId;
use std::collections::BTreeSet;

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
        if hakari.output_map.is_empty() {
            Ok(())
        } else {
            let mut dependency_ids = BTreeSet::new();

            for ((_, package_id), v) in &hakari.computed_map {
                for (_, inner_map) in v.inner_maps() {
                    if inner_map.len() > 1 {
                        dependency_ids.insert(*package_id);
                    }
                }
            }
            Err(VerifyErrors {
                hakari,
                dependency_ids,
            })
        }
    }
}

/// Context for errors returned by [`HakariBuilder::verify`].
///
/// For more about how verification works, see the documentation for the [`verify`](crate::verify)
/// module.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct VerifyErrors<'g> {
    /// The Hakari instance used to compute the errors.
    ///
    /// This is a special "verify mode" instance; for more about it, see the documentation for the
    /// [`verify`](crate::verify) module.
    pub hakari: Hakari<'g>,

    /// The dependency package IDs that were built with more than one feature set.
    pub dependency_ids: BTreeSet<&'g PackageId>,
}

impl<'g> VerifyErrors<'g> {
    /// Returns individual verification errors as [`HakariExplain`] instances.
    pub fn errors<'a>(
        &'a self,
    ) -> impl Iterator<Item = HakariExplain<'g, 'a>> + ExactSizeIterator + 'a {
        let hakari = &self.hakari;
        self.dependency_ids
            .iter()
            .copied()
            .map(move |id| HakariExplain::new(hakari, id).expect("package ID is from this graph"))
    }

    /// Returns a displayer for this instance.
    #[cfg(feature = "cli-support")]
    pub fn display<'verify>(&'verify self) -> VerifyErrorsDisplay<'g, 'verify> {
        VerifyErrorsDisplay::new(self)
    }
}

#[cfg(test)]
#[cfg(feature = "cli-support")]
mod cli_support_tests {
    use crate::summaries::{HakariConfig, DEFAULT_CONFIG_PATH};
    use guppy::MetadataCommand;

    /// Verify that this repo's `workspace-hack` works correctly.
    #[test]
    fn cargo_guppy_verify() {
        let graph = MetadataCommand::new()
            .build_graph()
            .expect("package graph built correctly");
        let config_path = graph.workspace().root().join(DEFAULT_CONFIG_PATH);
        let config_str = std::fs::read_to_string(&config_path).unwrap_or_else(|err| {
            panic!("could not read hakari config at {}: {}", config_path, err)
        });
        let config: HakariConfig = config_str.parse().unwrap_or_else(|err| {
            panic!(
                "could not deserialize hakari config at {}: {}",
                config_path, err
            )
        });

        let builder = config.builder.to_hakari_builder(&graph).unwrap();
        if let Err(errs) = builder.verify() {
            panic!("verify failed: {}", errs.display());
        }
    }
}
