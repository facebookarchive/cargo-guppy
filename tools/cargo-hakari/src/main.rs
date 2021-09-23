// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use cargo_hakari::Args;
use color_eyre::eyre::Result;
use structopt::StructOpt;

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::from_args();
    std::process::exit(args.exec()?)
}
