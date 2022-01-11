// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use cargo_hakari::Args;
use cfg_if::cfg_if;
use clap::Parser;
use color_eyre::eyre::Result;

// On Unix-like operating systems, the executable name of the Cargo subcommand usually doesn't have
// a file extension, while on Windows, executables usually have a ".exe" extension.
fn executable_name(subcommand: &str) -> String {
    cfg_if! {
        if #[cfg(target_os = "windows")] {
            format!("cargo-{}.exe", subcommand)
        } else {
            format!("cargo-{}", subcommand)
        }
    }
}

// When invoked as a cargo subcommand, cargo passes too many arguments so we need to filter out
// arg[1] if it matches the end of arg[0], e.i. "cargo-X X foo" should become "cargo-X foo".
fn args() -> impl Iterator<Item = String> {
    let mut args: Vec<String> = ::std::env::args().collect();

    if args.len() >= 2 && args[0].ends_with(&executable_name(&args[1])) {
        args.remove(1);
    }

    args.into_iter()
}

fn main() -> Result<()> {
    color_eyre::install()?;
    // Attempt to turn on ANSI color support on Windows. This may or may not work.
    let _ = enable_ansi_support::enable_ansi_support();

    let args = Args::parse_from(args());
    std::process::exit(args.exec()?)
}
