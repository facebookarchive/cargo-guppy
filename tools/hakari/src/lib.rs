// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

#![warn(missing_docs)]

//! Manage workspace-hack packages.
//!
//! This crate provides: TODO

mod cargo_toml;
mod hakari;
#[cfg(feature = "proptest010")]
mod proptest_helpers;
#[cfg(feature = "summaries")]
pub mod summaries;
mod toml_out;

pub use crate::{cargo_toml::*, hakari::*, toml_out::*};

/// Re-export diffy.
pub use diffy;
