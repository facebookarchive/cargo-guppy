// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Track and query Cargo dependency graphs.
//!
//! `guppy` provides a Rust interface to run queries over Cargo dependency graphs. `guppy` parses
//! the output of  [`cargo metadata`](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html),
//! then presents a graph interface over it..
//!
//! # Usage
//!
//! Add the following to `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! guppy = "0.1"
//! ```
//!
//! # Examples
//!
//! Print out all direct and transitive dependencies of a package:
//!
//! ```
//! use guppy::graph::PackageGraph;
//! use cargo_metadata::PackageId;
//! use std::iter;
//!
//! // `guppy` accepts `cargo metadata` JSON output. Use a pre-existing fixture for these examples.
//! let fixture = include_str!("../fixtures/metadata1.json");
//! let package_graph = PackageGraph::from_json(fixture).unwrap();
//!
//! // `guppy` provides several ways to get hold of package IDs. Use a pre-defined one for this
//! // example.
//! let package_id = PackageId { repr: "testcrate 0.1.0 (path+file:///fakepath/testcrate)".into() };
//! // dep_links returns all direct dependencies of a package, and it returns `None` if the package
//! // ID isn't recognized.
//! for link in package_graph.dep_links(&package_id).unwrap() {
//!     // A dependency link contains `from`, `to` and `edge`. The edge has information about e.g.
//!     // whether this is a build dependency.
//!     println!("direct dependency: {}", link.to.id());
//! }
//!
//! // Transitive dependencies are obtained through the `select_` APIs. They are always presented in
//! // topological order.
//! let select = package_graph.select_transitive_deps(iter::once(&package_id)).unwrap();
//! for dep_id in select.into_iter_ids(None) {
//!     // The select API also has an `into_iter_links()` method which returns links instead of IDs.
//!     println!("transitive dependency: {}", dep_id);
//! }
//! ```
//!
//! Remove all links that are dev-only, except for links within workspace packages.
//!
//! ```
//! use guppy::graph::PackageGraph;
//!
//! // `guppy` accepts `cargo metadata` JSON output. Use a pre-existing fixture for these examples.
//! let fixture = include_str!("../fixtures/metadata_libra.json");
//! let mut package_graph = PackageGraph::from_json(fixture).unwrap();
//!
//! // `retain_edges` takes a closure that returns `true` if this edge should be kept in the graph.
//! package_graph.retain_edges(|data, link| {
//!     // 'data' contains metadata for every package. It isn't used in this example but some
//!     // complex filters may make use of it.
//!     if link.from.in_workspace() && link.to.in_workspace() {
//!         return true;
//!     }
//!     !link.edge.dev_only()
//! });
//!
//! // Iterate over all links and assert that there are no dev-only links.
//! for link in package_graph.select_all().into_iter_links(None) {
//!     if !link.from.in_workspace() || !link.to.in_workspace() {
//!         assert!(!link.edge.dev_only());
//!     }
//! }
//! ```
//!
//! Print out a `dot` graph representing workspace packages, for formatting with
//! [graphviz](https://www.graphviz.org/).
//!
//! ```
//! use guppy::graph::{DependencyLink, DotWrite, PackageDotVisitor, PackageGraph, PackageMetadata};
//! use std::fmt;
//!
//! // `guppy` accepts `cargo metadata` JSON output. Use a pre-existing fixture for these examples.
//! let fixture = include_str!("../fixtures/metadata_libra.json");
//! let package_graph = PackageGraph::from_json(fixture).unwrap();
//!
//! // Non-workspace packages cannot depend on packages within the workspace, so the reverse
//! // transitive deps of workspace packages are exactly the set of workspace packages.
//! let select = package_graph
//!     .select_transitive_reverse_deps(package_graph.workspace().member_ids())
//!     .unwrap();
//!
//! // Define a visitor, which specifies what strings to print out for the graph.
//! struct PackageNameVisitor;
//!
//! impl PackageDotVisitor for PackageNameVisitor {
//!     fn visit_package(&self, package: &PackageMetadata, mut f: DotWrite<'_, '_>) -> fmt::Result {
//!         write!(f, "{}", package.name())
//!     }
//!
//!     fn visit_link(&self, link: DependencyLink<'_>, f: DotWrite<'_, '_>) -> fmt::Result {
//!         // Don't print out anything for links. One could print out e.g. whether this is
//!         // a dev-only link.
//!         Ok(())
//!     }
//! }
//!
//! // select.into_dot() implements `std::fmt::Display`, so it can be written out to a file, a
//! // string, stdout, etc.
//! let dot_graph = format!("{}", select.into_dot(PackageNameVisitor));
//! ```

pub mod config;
pub mod diff;
mod errors;
pub mod graph;
pub mod lockfile;
pub(crate) mod petgraph_support;
#[cfg(test)]
mod unit_tests;

pub use errors::Error;
