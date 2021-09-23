// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Manage workspace-hack crates.
//!
//! # Configuration
//!
//! `cargo-hakari` is configured through `Hakari.toml` at the root of the workspace.
//!
//! Example configuration:
//!
//! ```toml
//! # Which version of the Cargo resolver to use. This should match the "resolver" key in
//! # `Cargo.toml`.
//! resolver = "2"
//! # The name of the package used for workspace-hack unification.
//! hakari-package = "workspace-hack"
//!
//! # The platforms to perform resolution on. Can be left blank to perform a unified resolution
//! # across all platforms. (This may lead to potentially unexpected results.)
//! platforms = [
//!     { triple = "x86_64-unknown-linux-gnu", target-features = "unknown" }
//! ]
//!
//! # Options to control Hakari output.
//! [output]
//! # Write out exact versions rather than specifications. Set this to true if version numbers in
//! # `Cargo.toml` and `Cargo.lock` files are kept in sync, e.g. in some configurations of
//! # https://dependabot.com/.
//! # exact-versions = false
//!
//! # Write out a summary of builder options as a comment in the workspace-hack Cargo.toml.
//! # builder-summary = false
//! ```
//!
//! For more options, see the (TODO XXX configuration.md).

mod cargo_cli;
mod command;
mod output;

pub use command::Args;
