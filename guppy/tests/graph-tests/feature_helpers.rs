// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use guppy::graph::feature::{FeatureList, FeatureSet};
use guppy::PackageId;

pub(super) fn assert_features_for_package(
    feature_set: &FeatureSet<'_>,
    package_id: &PackageId,
    expected: &[Option<&str>],
    msg: &str,
) {
    let actual = feature_set
        .features_for(package_id)
        .expect("valid package ID");
    let expected = FeatureList::new(
        feature_set
            .graph()
            .package_graph()
            .metadata(package_id)
            .expect("valid package ID"),
        expected.iter().copied(),
    );

    assert_eq!(
        actual,
        Some(expected),
        "{}: for package {}, features in feature set match",
        msg,
        package_id
    );
}
