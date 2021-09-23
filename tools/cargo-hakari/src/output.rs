// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use colored::Colorize;
use env_logger::fmt::Formatter;
use log::{Level, LevelFilter, Record};
use std::io::Write;
use structopt::StructOpt;

#[derive(Copy, Clone, Debug, StructOpt)]
#[must_use]
pub(crate) struct OutputOpts {
    /// Suppress output
    #[structopt(
        name = "outputquiet",
        global = true,
        long = "quiet",
        short = "q",
        conflicts_with = "outputverbose"
    )]
    pub(crate) quiet: bool,
    /// Produce extra output
    #[structopt(
        name = "outputverbose",
        global = true,
        long = "verbose",
        short = "v",
        conflicts_with = "outputquiet"
    )]
    pub(crate) verbose: bool,

    /// Produce color output
    #[structopt(
        long,
        global = true,
        default_value = "auto",
        possible_values = &["auto", "always", "never"],
    )]
    pub(crate) color: Color,
}

impl OutputOpts {
    pub(crate) fn init_logger(&self) {
        let level = if self.quiet {
            LevelFilter::Error
        } else if self.verbose {
            LevelFilter::Debug
        } else {
            LevelFilter::Info
        };

        self.color.init_colored();

        env_logger::Builder::from_default_env()
            .filter_level(level)
            .format(format_fn)
            .init();
    }

    pub(crate) fn should_colorize(&self) -> bool {
        colored::control::SHOULD_COLORIZE.should_colorize()
    }
}

fn format_fn(f: &mut Formatter, record: &Record<'_>) -> std::io::Result<()> {
    match record.level() {
        Level::Error => writeln!(f, "{} {}", "error:".bold().red(), record.args()),
        Level::Warn => writeln!(f, "{} {}", "warning:".bold().yellow(), record.args()),
        Level::Info => writeln!(f, "{} {}", "info:".bold(), record.args()),
        Level::Debug => writeln!(f, "{} {}", "debug:".bold(), record.args()),
        _other => Ok(()),
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[must_use]
pub enum Color {
    Auto,
    Always,
    Never,
}

impl Color {
    fn init_colored(self) {
        match self {
            Color::Auto => colored::control::unset_override(),
            Color::Always => colored::control::set_override(true),
            Color::Never => colored::control::set_override(false),
        }
    }

    pub(crate) fn to_arg(self) -> &'static str {
        match self {
            Color::Auto => "--color=auto",
            Color::Always => "--color=always",
            Color::Never => "--color=never",
        }
    }
}

impl std::str::FromStr for Color {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto" => Ok(Color::Auto),
            "always" => Ok(Color::Always),
            "never" => Ok(Color::Never),
            s => Err(format!(
                "{} is not a valid option, expected `auto`, `always` or `never`",
                s
            )),
        }
    }
}
