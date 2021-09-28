// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Support for platform-specific resolution.
//!
//! `guppy` provides for target-specific dependencies.

mod platform_spec;
#[cfg(feature = "proptest1")]
mod proptest_helpers;
#[cfg(feature = "summaries")]
mod summaries;

pub use platform_spec::*;
#[cfg(feature = "summaries")]
pub use summaries::*;
