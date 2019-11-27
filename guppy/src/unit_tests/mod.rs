// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(feature = "proptest09")]
#[macro_use]
mod proptest_helpers;

#[cfg(not(feature = "proptest09"))]
macro_rules! proptest_suite {
    ($name: ident) => {
        // Empty macro to skip proptests if the proptest feature is disabled.
    };
}

mod dep_helpers;
mod dot_tests;
mod fixtures;
mod graph_tests;
mod reversed_tests;
