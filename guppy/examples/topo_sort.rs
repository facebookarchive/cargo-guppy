// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Print out crates in a workspace in topological order.
//!
//! The into_iter_ids and into_iter_metadatas iterators return packages in topological order. Note
//! that into_iter_links returns links in "link order" -- see its documentation for more.

use guppy::graph::{DependencyDirection, PackageGraph};
use guppy::Error;

fn main() -> Result<(), Error> {
    // `guppy` accepts `cargo metadata` JSON output. Use a pre-existing fixture for these examples.
    let fixture = include_str!("../fixtures/large/metadata_libra.json");
    let package_graph = PackageGraph::from_json(fixture)?;

    // Non-workspace packages cannot depend on packages within the workspace, so the reverse
    // transitive deps of workspace packages are exactly the set of workspace packages.
    let query = package_graph.query_reverse(package_graph.workspace().member_ids())?;
    let package_set = query.resolve();

    // Iterate over packages in forward topo order.
    for package in package_set.packages(DependencyDirection::Forward) {
        // All selected packages are in the workspace.
        let workspace_path = package
            .workspace_path()
            .expect("packages in workspace should have workspace path");
        println!("{}: {:?}", package.name(), workspace_path);
    }

    Ok(())
}
