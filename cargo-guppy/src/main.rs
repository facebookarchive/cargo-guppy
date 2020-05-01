// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use cargo_guppy::{CmdSelectOptions, DupsOptions, ResolveCargoOptions, SubtreeSizeOptions};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(about = "Cargo.lock file analysis")]
struct Args {
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    #[structopt(name = "diff")]
    /// Perform a diff of two Cargo.lock files
    Diff {
        #[structopt(long)]
        json: bool,
        old: String,
        new: String,
    },
    #[structopt(name = "dups")]
    /// Print the number of duplicate packages
    Duplicates(DupsOptions),
    #[structopt(name = "resolve-cargo")]
    /// Return packages and features that would be built by Cargo
    ResolveCargo(ResolveCargoOptions),
    #[structopt(name = "select")]
    /// Select packages and their transitive dependencies
    Select(CmdSelectOptions),
    #[structopt(name = "subtree-size")]
    /// Print a list of dependencies along with their unique subtree size
    SubtreeSize(SubtreeSizeOptions),
}

// When invoked as a cargo subcommand, cargo passes too many arguments so we need to filter out
// arg[1] if it matches the end of arg[0], e.i. "cargo-X X foo" should become "cargo-X foo".
fn args() -> impl Iterator<Item = String> {
    let mut args: Vec<String> = ::std::env::args().collect();

    if args.len() >= 2 && args[0].ends_with(&format!("cargo-{}", args[1])) {
        args.remove(1);
    }

    args.into_iter()
}

fn main() {
    let args = Args::from_iter(args());

    let result = match args.cmd {
        Command::Diff { json, old, new } => cargo_guppy::cmd_diff(json, &old, &new),
        Command::Duplicates(ref options) => cargo_guppy::cmd_dups(options),
        Command::ResolveCargo(ref options) => cargo_guppy::cmd_resolve_cargo(options),
        Command::Select(ref options) => cargo_guppy::cmd_select(options),
        Command::SubtreeSize(ref options) => cargo_guppy::cmd_subtree_size(options),
    };

    match result {
        Err(e) => println!("{}\nAborting...", e),
        Ok(()) => {}
    }
}
