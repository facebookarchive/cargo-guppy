// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::Result;
use fixtures::json::JsonFixture;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct FixtureManager {
    // TODO: add global options
    #[structopt(subcommand)]
    cmd: Command,
}

impl FixtureManager {
    pub fn exec(self) -> Result<()> {
        match self.cmd {
            Command::List => list(),
        }
    }
}

#[derive(Debug, StructOpt)]
enum Command {
    #[structopt(name = "list")]
    /// List fixtures
    List,
}

pub fn list() -> Result<()> {
    for (name, fixture) in JsonFixture::all_fixtures().iter() {
        println!("{}: {}", name, fixture.workspace_path().display());
    }

    Ok(())
}
