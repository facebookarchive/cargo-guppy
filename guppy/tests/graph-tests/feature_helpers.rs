// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use guppy::{
    graph::feature::{FeatureLabel, FeatureSet},
    PackageId,
};

pub(super) fn assert_features_for_package(
    feature_set: &FeatureSet<'_>,
    package_id: &PackageId,
    expected: Option<&[FeatureLabel<'_>]>,
    msg: &str,
) {
    let actual = feature_set
        .features_for(package_id)
        .expect("valid package ID");

    assert_eq!(
        actual.as_ref().map(|list| list.labels()),
        expected,
        "{}: for package {}, features in feature set match",
        msg,
        package_id
    );
}
