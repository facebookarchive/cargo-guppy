// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Track and query Cargo dependency graphs.
//!
//! `guppy` provides a Rust interface to run queries over Cargo dependency graphs. `guppy` parses
//! the output of  [`cargo metadata`](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html),
//! then presents a graph interface over it.
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
//! Print out all direct dependencies of a package:
//!
//! ```
//! use guppy::graph::PackageGraph;
//! use cargo_metadata::PackageId;
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
//! ```
//!
//! For more examples, see
//! [the `examples` directory](https://github.com/calibra/cargo-guppy/tree/master/guppy/examples).

pub mod config;
pub mod diff;
mod errors;
pub mod graph;
pub mod lockfile;
pub(crate) mod petgraph_support;
#[cfg(test)]
mod unit_tests;

pub use errors::Error;
