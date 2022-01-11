// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use cargo_compare::CargoCompare;
use clap::Parser;
use color_eyre::eyre::Result;

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = CargoCompare::parse();
    args.exec()
}
