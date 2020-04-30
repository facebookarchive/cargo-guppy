// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::common::GuppyCargoCommon;
use anyhow::Result;
use diffus::{edit, Diffable};
use guppy::{MetadataCommand, PackageId};
use std::collections::{BTreeMap, BTreeSet};
use structopt::StructOpt;

/// Options for cargo/guppy comparisons.
#[derive(Debug, StructOpt)]
pub struct DiffOpts {
    #[structopt(flatten)]
    common: GuppyCargoCommon,
}

impl DiffOpts {
    /// Executes this command.
    pub fn exec(self) -> Result<()> {
        let cargo_map = self.common.resolve_cargo()?;
        let graph = MetadataCommand::new().build_graph()?;
        let guppy_map = self.common.resolve_guppy(&graph)?;

        // As of 2020-04-30, Cargo's APIs don't let users tell the difference between the package
        // being missing entirely, and the package being present but with no features.
        //
        // Note that this is only a problem for the v2 resolver -- the v1 resolver unifies
        // everything across the target and host anyway so this issue is moot there.
        //
        // XXX fix this upstream.
        let ignore_inserts = self.common.v2;

        println!("** target diff (guppy -> cargo):");
        print_diff(&guppy_map.target_map, &cargo_map.target_map, ignore_inserts);

        println!("\n** host diff (guppy -> cargo):");
        print_diff(&guppy_map.host_map, &cargo_map.host_map, ignore_inserts);

        Ok(())
    }
}

fn print_diff(
    a: &BTreeMap<PackageId, BTreeSet<String>>,
    b: &BTreeMap<PackageId, BTreeSet<String>>,
    ignore_inserts: bool,
) {
    if let edit::Edit::Change(diff) = a.diff(&b) {
        for (pkg_id, diff) in diff {
            let ignore = diff.is_copy() || (ignore_inserts && diff.is_insert());
            if !ignore {
                println!("{}: {:?}", pkg_id, diff);
            }
        }
    }
}
