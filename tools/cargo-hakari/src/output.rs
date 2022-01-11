// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
use env_logger::fmt::Formatter;
use log::{Level, LevelFilter, Record};
use owo_colors::{OwoColorize, Stream, Style};
use std::{io::Write, sync::Arc};

#[derive(Debug, Parser)]
#[must_use]
pub(crate) struct OutputOpts {
    /// Suppress output
    #[clap(
        name = "outputquiet",
        global = true,
        long = "quiet",
        short = 'q',
        conflicts_with = "outputverbose"
    )]
    pub(crate) quiet: bool,
    /// Produce extra output
    #[clap(
        name = "outputverbose",
        global = true,
        long = "verbose",
        short = 'v',
        conflicts_with = "outputquiet"
    )]
    pub(crate) verbose: bool,

    /// Produce color output
    #[clap(
        long,
        global = true,
        default_value = "auto",
        possible_values = &["auto", "always", "never"],
    )]
    pub(crate) color: Color,
}

impl OutputOpts {
    pub(crate) fn init(self) -> OutputContext {
        let OutputOpts {
            quiet,
            verbose,
            color,
        } = self;
        let level = if quiet {
            LevelFilter::Error
        } else if verbose {
            LevelFilter::Debug
        } else {
            LevelFilter::Info
        };

        color.init_colored();

        let mut styles = Styles::default();
        if stderr_supports_color() {
            styles.colorize();
        }

        env_logger::Builder::from_default_env()
            .filter_level(level)
            .format(format_fn)
            .init();

        OutputContext {
            quiet,
            verbose,
            color,
            styles: Arc::new(styles),
        }
    }
}

#[derive(Clone, Debug)]
#[must_use]
pub(crate) struct OutputContext {
    pub(crate) quiet: bool,
    pub(crate) verbose: bool,
    pub(crate) color: Color,
    pub(crate) styles: Arc<Styles>,
}

fn format_fn(f: &mut Formatter, record: &Record<'_>) -> std::io::Result<()> {
    match record.level() {
        Level::Error => writeln!(
            f,
            "{} {}",
            "error:".if_supports_color(Stream::Stderr, |s| s.style(Style::new().bold().red())),
            record.args()
        ),
        Level::Warn => writeln!(
            f,
            "{} {}",
            "warning:".if_supports_color(Stream::Stderr, |s| s.style(Style::new().bold().yellow())),
            record.args()
        ),
        Level::Info => writeln!(
            f,
            "{} {}",
            "info:".if_supports_color(Stream::Stderr, |s| s.bold()),
            record.args()
        ),
        Level::Debug => writeln!(
            f,
            "{} {}",
            "debug:".if_supports_color(Stream::Stderr, |s| s.bold()),
            record.args()
        ),
        _other => Ok(()),
    }
}

fn stderr_supports_color() -> bool {
    match supports_color::on_cached(Stream::Stderr) {
        Some(level) => level.has_basic,
        None => false,
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
            Color::Auto => owo_colors::unset_override(),
            Color::Always => owo_colors::set_override(true),
            Color::Never => owo_colors::set_override(false),
        }
    }

    pub(crate) fn is_enabled(self) -> bool {
        match self {
            // Currently, all output from cargo-hakari goes to stderr.
            Color::Auto => stderr_supports_color(),
            Color::Always => true,
            Color::Never => false,
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

#[derive(Clone, Debug, Default)]
pub(crate) struct Styles {
    pub(crate) config_path: Style,
    pub(crate) command: Style,
    pub(crate) registry_url: Style,
    pub(crate) package_name: Style,
    pub(crate) package_version: Style,
}

impl Styles {
    fn colorize(&mut self) {
        self.config_path = Style::new().blue().bold();
        self.command = Style::new().bold();
        self.registry_url = Style::new().magenta().bold();
        self.package_name = Style::new().bold();
        self.package_version = Style::new().bold();
    }
}
