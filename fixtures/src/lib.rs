// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Test fixtures for guppy.

pub mod dep_helpers;
pub mod details;
pub mod json;

use guppy::PackageId;

/// Helper for creating `PackageId` instances in test code.
pub fn package_id(s: impl Into<Box<str>>) -> PackageId {
    PackageId::new(s)
}
