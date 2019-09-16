// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::fixtures::{self, Fixture};
use cargo_metadata::PackageId;

// Test specific details extracted from metadata1.json.
#[test]
fn metadata1() {
    let metadata1 = Fixture::metadata1();
    metadata1.verify();

    let graph = metadata1.graph();
    let mut root_deps: Vec<_> = graph
        .deps(&PackageId {
            repr: fixtures::METADATA1_TESTCRATE.into(),
        })
        .collect();

    assert_eq!(root_deps.len(), 1, "the root crate has one dependency");
    let dep = root_deps.pop().expect("the root crate has one dependency");
    // XXX test for details of dependency edges as well?
    assert!(dep.edge.normal().is_some(), "normal dependency is defined");
    assert!(dep.edge.build().is_some(), "build dependency is defined");
    assert!(dep.edge.dev().is_some(), "dev dependency is defined");
}

#[test]
fn metadata2() {
    let metadata2 = Fixture::metadata2();
    metadata2.verify();
}
