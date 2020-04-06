// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Print out direct and transitive dependencies of a package.

use guppy::graph::{DependencyDirection, PackageGraph};
use guppy::{Error, PackageId};
use std::iter;

fn main() -> Result<(), Error> {
    // `guppy` accepts `cargo metadata` JSON output. Use a pre-existing fixture for these examples.
    let fixture = include_str!("../fixtures/small/metadata1.json");
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
    let select = package_graph.select_forward(iter::once(&package_id))?;
    let resolve = select.resolve();
    for dep_id in resolve.into_ids(DependencyDirection::Forward) {
        // The select API also has an `into_iter_links()` method which returns links instead of IDs.
        println!("transitive: {}", dep_id);
    }
    Ok(())
}
