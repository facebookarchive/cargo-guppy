// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Support for comparing Cargo and Guppy.

use crate::check::CheckOpts;
use crate::diff::DiffOpts;
use anyhow::Result;
use either::Either;
use guppy::graph::PackageGraph;
use std::env;
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use tempfile::TempDir;

pub mod check;
pub mod common;
pub mod diff;
#[cfg(test)]
mod tests;
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
            Command::Diff(opts) => {
                // Don't use the temporary home here so that Cargo caches can be reused.
                let graph = opts.common.metadata_opts.make_command().build_graph()?;
                let ctx = GlobalContext::new(false, &graph)?;
                opts.exec(&ctx)
            }
            Command::Check(opts) => {
                // Don't use the temporary home here so that Cargo caches can be reused.
                let graph = opts.metadata.make_command().build_graph()?;
                let ctx = GlobalContext::new(false, &graph)?;
                opts.exec(&ctx)
            }
        }
    }
}

#[derive(Debug, StructOpt)]
enum Command {
    #[structopt(name = "diff")]
    /// Perform a diff of Cargo's results against Guppy's
    Diff(DiffOpts),
    /// Generate many queries and compare Cargo and Guppy
    Check(CheckOpts),
}

/// Global context for Cargo comparisons.
#[derive(Debug)]
pub struct GlobalContext<'g> {
    home_dir: Either<TempDir, PathBuf>,
    graph: &'g PackageGraph,
}

impl<'g> GlobalContext<'g> {
    pub fn new(temp_home: bool, graph: &'g PackageGraph) -> Result<Self> {
        let home = if temp_home {
            Either::Left(TempDir::new()?)
        } else {
            Either::Right(env::current_dir()?)
        };
        Ok(Self {
            home_dir: home,
            graph,
        })
    }

    pub fn home_dir(&self) -> &Path {
        match &self.home_dir {
            Either::Left(temp_home) => temp_home.path(),
            Either::Right(home_dir) => home_dir.as_path(),
        }
    }

    pub fn graph(&self) -> &'g PackageGraph {
        self.graph
    }
}
