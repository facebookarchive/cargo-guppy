// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::fixtures;
use crate::graph::PackageGraph;

// Test specific details extracted from metadata1.json.
#[test]
fn metadata1() {
    let metadata = fixtures::parse_metadata(fixtures::METADATA1);
    let details = fixtures::FixtureDetails::metadata1();

    let graph = PackageGraph::new(metadata).expect("constructing package graph should succeed");
    let mut workspace_members = graph.workspace_members();
    assert_eq!(
        workspace_members.len(),
        1,
        "this is a single-crate metadata, so the workspace has one member"
    );
    let root = workspace_members.next().unwrap();
    let root_metadata = graph.metadata(root).expect("root package metadata");
    details.assert_metadata(fixtures::METADATA1_TESTCRATE, root_metadata, "root package");

    let mut root_deps = graph.deps(root).collect::<Vec<_>>();
    details.assert_dependencies(
        fixtures::METADATA1_TESTCRATE,
        root_deps.clone(),
        "testcrate dependencies",
    );

    assert_eq!(root_deps.len(), 1, "the root crate has one dependency");
    let dep = root_deps.pop().expect("the root crate has one dependency");
    // XXX test for details of dependency edges as well?
    assert!(dep.edge.normal().is_some(), "normal dependency is defined");
    assert!(dep.edge.build().is_some(), "build dependency is defined");
    assert!(dep.edge.dev().is_some(), "dev dependency is defined");

    // Test the dependencies of datatest.
    let datatest_id = dep.to.id();
    details.assert_dependencies(
        fixtures::METADATA1_DATATEST,
        graph.deps(datatest_id),
        "datatest dependencies",
    );
}
