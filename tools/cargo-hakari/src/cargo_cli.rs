// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cargo CLI support.

use crate::output::OutputOpts;
use camino::Utf8PathBuf;
use std::{convert::TryInto, env, path::PathBuf};

#[derive(Clone, Debug)]
pub(crate) struct CargoCli<'a> {
    cargo_path: Utf8PathBuf,
    output_opts: OutputOpts,
    command: &'a str,
    args: Vec<&'a str>,
}

impl<'a> CargoCli<'a> {
    pub(crate) fn new(command: &'a str, output_opts: OutputOpts) -> Self {
        let cargo_path = cargo_path();
        Self {
            cargo_path,
            output_opts,
            command,
            args: vec![],
        }
    }

    pub(crate) fn add_arg(&mut self, arg: &'a str) -> &mut Self {
        self.args.push(arg);
        self
    }

    pub(crate) fn add_args(&mut self, args: impl IntoIterator<Item = &'a str>) -> &mut Self {
        self.args.extend(args);
        self
    }

    pub(crate) fn all_args(&self) -> Vec<&str> {
        let mut all_args = vec![self.cargo_path.as_str(), self.command];
        all_args.extend_from_slice(&self.args);
        all_args
    }

    pub(crate) fn to_expression(&self) -> duct::Expression {
        let mut initial_args = vec![];
        if self.output_opts.quiet {
            initial_args.push("--quiet");
        }
        if self.output_opts.verbose {
            initial_args.push("--verbose");
        }
        initial_args.push(self.output_opts.color.to_arg());

        initial_args.push(self.command);

        duct::cmd(
            self.cargo_path.as_std_path(),
            initial_args.into_iter().chain(self.args.iter().copied()),
        )
    }
}

fn cargo_path() -> Utf8PathBuf {
    match env::var_os("CARGO") {
        Some(cargo_path) => PathBuf::from(cargo_path)
            .try_into()
            .expect("CARGO env var is not valid UTF-8"),
        None => Utf8PathBuf::from("cargo"),
    }
}
