// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Support for comparing Cargo and Guppy.

use crate::diff::DiffOpts;
use anyhow::Result;
use structopt::StructOpt;

pub mod common;
pub mod diff;
pub mod type_conversions;

#[derive(Debug, StructOpt)]
pub struct CargoCompare {
    // TODO: add global options
    #[structopt(subcommand)]
    cmd: Command,
}

impl CargoCompare {
    pub fn exec(self) -> Result<()> {
        match self.cmd {
            Command::Diff(opts) => opts.exec(),
        }
    }
}

#[derive(Debug, StructOpt)]
enum Command {
    #[structopt(name = "diff")]
    /// Perform a diff of Cargo's results against Guppy's
    Diff(DiffOpts),
}
