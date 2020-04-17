// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::Result;
use cargo_compare::CargoCompare;
use structopt::StructOpt;

fn main() -> Result<()> {
    let args = CargoCompare::from_args();
    args.exec()
}
