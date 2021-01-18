// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Type conversions between cargo and guppy.

use std::collections::BTreeSet;

pub trait ToGuppy {
    type Guppy;

    fn to_guppy(&self) -> Self::Guppy;
}

impl ToGuppy for cargo::core::PackageId {
    type Guppy = guppy::PackageId;

    fn to_guppy(&self) -> Self::Guppy {
        // This is the same format as the Serialize impl of cargo's PackageId.
        guppy::PackageId::new(format!(
            "{} {} ({})",
            self.name(),
            self.version(),
            self.source_id().as_url(),
        ))
    }
}

impl ToGuppy for Vec<cargo::util::interning::InternedString> {
    type Guppy = BTreeSet<String>;

    fn to_guppy(&self) -> Self::Guppy {
        self.iter().map(|s| s.to_string()).collect()
    }
}
