// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Command-line operations for `hakari`.
//!
//! These are primarily intended for use with `cargo hakari`, but may be used by other command-line
//! frontends.
//!
//! Requires the `cli-support` feature to be enabled.

mod initialize;
mod manage_deps;
mod workspace_ops;

pub use initialize::*;
pub use workspace_ops::*;
