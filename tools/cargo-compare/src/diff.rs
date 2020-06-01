// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::common::GuppyCargoCommon;
use crate::GlobalContext;
use anyhow::Result;
use diffus::{edit, Diffable};
use guppy::graph::PackageGraph;
use guppy::PackageId;
use itertools::Itertools;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use structopt::StructOpt;

/// Options for cargo/guppy comparisons.
#[derive(Debug, StructOpt)]
pub struct DiffOpts {
    #[structopt(flatten)]
    pub common: GuppyCargoCommon,
    /// Print out unchanged packages and features as well
    #[structopt(long)]
    pub verbose: bool,
}

impl DiffOpts {
    /// Executes this command.
    pub fn exec(self, ctx: &GlobalContext) -> Result<()> {
        let cargo_map = self.common.resolve_cargo(ctx)?;
        let guppy_map = self.common.resolve_guppy(ctx)?;

        let target_diff = FeatureDiff {
            graph: ctx.graph(),
            a: &guppy_map.target_map,
            b: &cargo_map.target_map,
            verbose: self.verbose,
        };
        println!("** target diff (guppy -> cargo):\n{}\n", target_diff);

        let host_diff = FeatureDiff {
            graph: ctx.graph(),
            a: &guppy_map.host_map,
            b: &cargo_map.host_map,
            verbose: self.verbose,
        };
        println!("** host diff (guppy -> cargo):\n{}", host_diff);

        Ok(())
    }
}

struct FeatureDiff<'g> {
    graph: &'g PackageGraph,
    a: &'g BTreeMap<PackageId, BTreeSet<String>>,
    b: &'g BTreeMap<PackageId, BTreeSet<String>>,
    verbose: bool,
}

impl<'g> fmt::Display for FeatureDiff<'g> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.a.diff(&self.b) {
            edit::Edit::Change(diff) => {
                for (pkg_id, diff) in diff {
                    use diffus::edit::map::Edit;

                    let package = self.graph.metadata(&pkg_id).expect("valid package ID");
                    match diff {
                        Edit::Copy(features) => {
                            if self.verbose {
                                writeln!(
                                    f,
                                    "{} {}: unchanged\n  * features: {}",
                                    package.name(),
                                    package.version(),
                                    features.iter().join(", ")
                                )?
                            }
                        }
                        Edit::Insert(features) => writeln!(
                            f,
                            "{} {}: added\n  * new features: {}",
                            package.name(),
                            package.version(),
                            features.iter().join(", ")
                        )?,
                        Edit::Remove(features) => writeln!(
                            f,
                            "{} {}: removed\n  * old features: {}",
                            package.name(),
                            package.version(),
                            features.iter().join(", "),
                        )?,
                        Edit::Change(diff) => {
                            writeln!(
                                f,
                                "{} {}: changed, features:",
                                package.name(),
                                package.version(),
                            )?;
                            for (feature_name, diff) in diff {
                                use diffus::edit::set::Edit;

                                match diff {
                                    Edit::Copy(_) => {
                                        if self.verbose {
                                            writeln!(f, "  * {}: unchanged", feature_name)?
                                        }
                                    }
                                    Edit::Insert(_) => {
                                        writeln!(f, "  * {}: added", feature_name)?;
                                    }
                                    Edit::Remove(_) => {
                                        writeln!(f, "  * {}: removed", feature_name)?;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            edit::Edit::Copy(map) => {
                if self.verbose {
                    for (pkg_id, features) in map {
                        let package = self.graph.metadata(&pkg_id).expect("valid package ID");

                        writeln!(
                            f,
                            "{} {}: unchanged\n  * features: {}",
                            package.name(),
                            package.version(),
                            features.iter().join(", ")
                        )?;
                    }
                }
            }
        }

        Ok(())
    }
}
