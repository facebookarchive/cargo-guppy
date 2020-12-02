// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Track and query Cargo dependency graphs.
//!
//! `guppy` provides a Rust interface to run queries over Cargo dependency graphs. `guppy` parses
//! the output of  [`cargo metadata`](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html),
//! then presents a graph interface over it.
//!
//! # Optional features
//!
//! * `proptest010`: Support for [property-based testing](https://jessitron.com/2013/04/25/property-based-testing-what-is-it/)
//!   using the [`proptest`](https://altsysrq.github.io/proptest-book/intro.html) framework.
//! * `rayon1`: Support for parallel iterators through [Rayon](docs.rs/rayon/1) (preliminary work
//!   so far, more parallel iterators to be added in the future).
//! * `summaries`: Support for writing out [build summaries](https://github.com/facebookincubator/cargo-guppy/tree/master/guppy-summaries).
//!
//! # Examples
//!
//! Print out all direct dependencies of a package:
//!
//! ```
//! use guppy::{CargoMetadata, PackageId};
//!
//! // `guppy` accepts `cargo metadata` JSON output. Use a pre-existing fixture for these examples.
//! let metadata = CargoMetadata::parse_json(include_str!("../../fixtures/small/metadata1.json")).unwrap();
//! let package_graph = metadata.build_graph().unwrap();
//!
//! // `guppy` provides several ways to get hold of package IDs. Use a pre-defined one for this
//! // example.
//! let package_id = PackageId::new("testcrate 0.1.0 (path+file:///fakepath/testcrate)");
//!
//! // The `metadata` method returns information about the package, or `None` if the package ID
//! // wasn't recognized.
//! let package = package_graph.metadata(&package_id).unwrap();
//!
//! // `direct_links` returns all direct dependencies of a package.
//! for link in package.direct_links() {
//!     // A dependency link contains `from()`, `to()` and information about the specifics of the
//!     // dependency.
//!     println!("direct dependency: {}", link.to().id());
//! }
//! ```
//!
//! For more examples, see
//! [the `examples` directory](https://github.com/facebookincubator/cargo-guppy/tree/master/guppy/examples).

#![warn(missing_docs)]

#[macro_use]
mod macros;

mod debug_ignore;
mod dependency_kind;
pub mod errors;
pub mod graph;
mod metadata_command;
mod obs;
mod package_id;
pub(crate) mod petgraph_support;
pub(crate) mod sorted_set;
#[cfg(test)]
mod unit_tests;

pub use dependency_kind::*;
pub use errors::Error;
pub use metadata_command::*;
pub use obs::*;
pub use package_id::PackageId;

// Public re-exports for upstream crates used in APIs. The no_inline ensures that they show up as
// re-exports in documentation.
#[doc(no_inline)]
pub use semver::Version;
#[doc(no_inline)]
pub use serde_json::Value as JsonValue;
// These are inlined -- generally, treat target_spec as a private dependency so expose these types
// as part of guppy's API.
pub use target_spec::{Error as TargetSpecError, Platform, TargetFeatures};
