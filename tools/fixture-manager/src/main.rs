// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::Result;
use fixture_manager::FixtureManager;
use structopt::StructOpt;

fn main() -> Result<()> {
    let args = FixtureManager::from_args();
    args.exec()
}
