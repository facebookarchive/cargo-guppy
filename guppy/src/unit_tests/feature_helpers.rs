// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::feature::FeatureSet;
use crate::PackageId;
use std::collections::HashSet;

pub(super) fn assert_features_for_package(
    feature_set: &FeatureSet<'_>,
    package_id: &PackageId,
    expected: &[Option<&str>],
    msg: &str,
) {
    let actual: HashSet<_> = feature_set
        .features_for(package_id)
        .expect("valid package ID")
        .collect();
    let expected: HashSet<_> = expected.iter().copied().collect();

    assert_eq!(
        actual, expected,
        "{}: for package {}, features in feature set match",
        msg, package_id
    );
}
