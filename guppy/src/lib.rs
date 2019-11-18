// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

pub mod config;
pub mod diff;
mod errors;
pub mod graph;
pub mod lockfile;
pub(crate) mod petgraph_support;
#[cfg(test)]
mod unit_tests;

pub use errors::Error;
