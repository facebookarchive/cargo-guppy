// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Print out direct and transitive dependencies of a package.

use cargo_metadata::PackageId;
use guppy::graph::PackageGraph;
use guppy::Error;
use std::iter;

fn main() -> Result<(), Error> {
    // `guppy` accepts `cargo metadata` JSON output. Use a pre-existing fixture for these examples.
    let fixture = include_str!("../fixtures/metadata1.json");
    let package_graph = PackageGraph::from_json(fixture)?;

    // `guppy` provides several ways to get hold of package IDs. Use a pre-defined one for this
    // example.
    let package_id = PackageId {
        repr: "testcrate 0.1.0 (path+file:///fakepath/testcrate)".into(),
    };
    // dep_links returns all direct dependencies of a package, and it returns `None` if the package
    // ID isn't recognized.
    for link in package_graph.dep_links(&package_id).unwrap() {
        // A dependency link contains `from`, `to` and `edge`. The edge has information about e.g.
        // whether this is a build dependency.
        println!("direct: {}", link.to.id());
    }

    // Transitive dependencies are obtained through the `select_` APIs. They are always presented in
    // topological order.
    let select = package_graph.select_transitive_deps(iter::once(&package_id))?;
    for dep_id in select.into_iter_ids(None) {
        // The select API also has an `into_iter_links()` method which returns links instead of IDs.
        println!("transitive: {}", dep_id);
    }
    Ok(())
}
