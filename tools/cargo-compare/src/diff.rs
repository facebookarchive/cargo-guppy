// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::common::GuppyCargoCommon;
use crate::GlobalContext;
use anyhow::Result;
use diffus::{edit, Diffable};
use guppy::PackageId;
use std::collections::{BTreeMap, BTreeSet};
use structopt::StructOpt;

/// Options for cargo/guppy comparisons.
#[derive(Debug, StructOpt)]
pub struct DiffOpts {
    #[structopt(flatten)]
    pub common: GuppyCargoCommon,
}

impl DiffOpts {
    /// Executes this command.
    pub fn exec(self, ctx: &GlobalContext) -> Result<()> {
        let cargo_map = self.common.resolve_cargo(ctx)?;
        let guppy_map = self.common.resolve_guppy(ctx)?;

        println!("** target diff (guppy -> cargo):");
        print_diff(&guppy_map.target_map, &cargo_map.target_map);

        println!("\n** host diff (guppy -> cargo):");
        print_diff(&guppy_map.host_map, &cargo_map.host_map);

        Ok(())
    }
}

fn print_diff(
    a: &BTreeMap<PackageId, BTreeSet<String>>,
    b: &BTreeMap<PackageId, BTreeSet<String>>,
) {
    if let edit::Edit::Change(diff) = a.diff(&b) {
        for (pkg_id, diff) in diff {
            if !diff.is_copy() {
                println!("{}: {:?}", pkg_id, diff);
            }
        }
    }
}
