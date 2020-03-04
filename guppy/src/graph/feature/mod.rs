// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Graph analysis for individual features within a package.
//!
//! `FeatureGraph` can be used to do a more precise analysis than is possible at the package level.
//! For example, an optional feature not included a default build can potentially pull in a large
//! number of extra dependencies. This module allows for those subgraphs to be filtered out.

mod build;
mod cycles;
mod graph_impl;
#[cfg(feature = "proptest09")]
mod proptest09;
mod select;

pub use cycles::*;
pub use graph_impl::*;
pub use select::*;
