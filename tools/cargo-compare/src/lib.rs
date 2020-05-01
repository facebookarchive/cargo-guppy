// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Support for comparing Cargo and Guppy.

use crate::diff::DiffOpts;
use anyhow::Result;
use either::Either;
use std::env;
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use tempfile::TempDir;

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
        // Don't use the temporary home here so that Cargo caches can be reused.
        let ctx = GlobalContext::new(false)?;

        match self.cmd {
            Command::Diff(opts) => opts.exec(&ctx),
        }
    }
}

#[derive(Debug, StructOpt)]
enum Command {
    #[structopt(name = "diff")]
    /// Perform a diff of Cargo's results against Guppy's
    Diff(DiffOpts),
}

/// Global context for Cargo comparisons.
#[derive(Debug)]
pub struct GlobalContext {
    home_dir: Either<TempDir, PathBuf>,
}

impl GlobalContext {
    pub fn new(temp_home: bool) -> Result<Self> {
        let home = if temp_home {
            Either::Left(TempDir::new()?)
        } else {
            Either::Right(env::current_dir()?)
        };
        Ok(Self { home_dir: home })
    }

    pub fn home_dir(&self) -> &Path {
        match &self.home_dir {
            Either::Left(temp_home) => temp_home.path(),
            Either::Right(home_dir) => home_dir.as_path(),
        }
    }
}
