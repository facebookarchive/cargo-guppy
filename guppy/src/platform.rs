// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use once_cell::sync::Lazy;
use platforms::guess_current;q

/// Represents a specific platform to evaluate targets against.

/// Returns the platform (target triple) that `guppy` believes it is running on.
///
/// This is not perfect, and may return `None` on some esoteric platforms.
///
/// The current platform is used to construct `PackageGraph` instances, so if this returns `None`,
/// `guppy` will not be able to construct them.
pub fn current_platform() -> Option<&'static str> {
    static CURRENT_PLATFORM: Lazy<Option<&str>> =
        Lazy::new(|| guess_current().map(|current| current.target_triple));

    *CURRENT_PLATFORM
}
