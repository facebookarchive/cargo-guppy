// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Print out a dot representation of a subgraph, for formatting with graphviz.
//!
//! This example prints out a dot representation of the dependencies between all packages in a
//! workspace. It skips over any non-workspace packages.
//!
//! Try running this example with graphviz:
//!
//! ```text
//! cargo run --example print_dot > graph.dot
//! dot -Tpng graph.dot -o graph.png
//! ```

use guppy::graph::{DependencyLink, DotWrite, PackageDotVisitor, PackageGraph, PackageMetadata};
use guppy::Error;
use std::fmt;

// Define a visitor, which specifies what strings to print out for the graph.
struct PackageNameVisitor;

impl PackageDotVisitor for PackageNameVisitor {
    fn visit_package(&self, package: &PackageMetadata, mut f: DotWrite<'_, '_>) -> fmt::Result {
        // Print out the name of the package. Other metadata can also be printed out.
        //
        // If you need to look at data for other packages, store a reference to the PackageGraph in
        // the visitor.
        write!(f, "{}", package.name())
    }

    fn visit_link(&self, link: DependencyLink<'_>, mut f: DotWrite<'_, '_>) -> fmt::Result {
        if link.edge.dev_only() {
            write!(f, "dev-only")
        } else {
            // Don't print out anything if this isn't a dev-only link.
            Ok(())
        }
    }
}

fn main() -> Result<(), Error> {
    // `guppy` accepts `cargo metadata` JSON output. Use a pre-existing fixture for these examples.
    let fixture = include_str!("../fixtures/metadata_libra.json");
    let package_graph = PackageGraph::from_json(fixture)?;

    // Non-workspace packages cannot depend on packages within the workspace, so the reverse
    // transitive deps of workspace packages are exactly the set of workspace packages.
    let select =
        package_graph.select_transitive_reverse_deps(package_graph.workspace().member_ids())?;

    // select.into_dot() implements `std::fmt::Display`, so it can be written out to a file, a
    // string, stdout, etc.
    println!("{}", select.into_dot(PackageNameVisitor));
    Ok(())
}
