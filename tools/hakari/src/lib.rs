// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

#![warn(missing_docs)]

//! `hakari` is the library underlying [`cargo hakari`](https://docs.rs/cargo-hakari/*), a tool to
//! manage `workspace-hack` packages.
//!
//! # Examples
//!
//! ```rust
//! use guppy::MetadataCommand;
//! use hakari::{HakariBuilder, HakariOutputOptions};
//!
//! // Use this workspace's PackageGraph for these tests.
//! let package_graph = MetadataCommand::new()
//!     .build_graph()
//!     .expect("obtained cargo-guppy's PackageGraph");
//! // The second argument to HakariBuilder::new specifies a Hakari (workspace-hack) package. At
//! // the moment cargo-guppy does not have such a package, and it is a TODO to add one.
//! let hakari_builder = HakariBuilder::new(&package_graph, None)
//!     .expect("HakariBuilder was constructed");
//!
//! // HakariBuilder has a number of config options. For this example, use the defaults.
//! let hakari = hakari_builder.compute();
//!
//! // "hakari" can be used to build a TOML representation that forms part of a Cargo.toml file.
//! // Existing Cargo.toml files can be managed using Hakari::read_toml.
//! let toml = hakari.to_toml_string(&HakariOutputOptions::default()).expect("TOML output was constructed");
//!
//! // toml contains the Cargo.toml [dependencies] that would go in the Hakari package. It can be
//! // written out through `HakariCargoToml` (returned by Hakari::read_toml) or manually.
//! println!("Cargo.toml contents:\n{}", toml);
//! ```
//!
//!
//! The `cargo-guppy` repository uses a workspace-hack crate managed by `cargo hakari`. [See the
//! generated `Cargo.toml`.](https://github.com/facebookincubator/cargo-guppy/blob/main/workspace-hack/Cargo.toml)
//!
//! The `cargo-guppy` repository also has a number of fixtures that demonstrate Hakari's output.
//! [Here is an example.](https://github.com/facebookincubator/cargo-guppy/blob/main/fixtures/guppy/hakari/metadata_guppy_869476c-1.toml)
//!
//! # How `hakari` works
//!
//! Hakari follows a three-step process.
//!
//! ## 1. Configuration
//!
//! A [`HakariBuilder`](HakariBuilder) provides options to configure how a Hakari computation is done. Options supported
//! include:
//! * [the location of the `workspace-hack` package](HakariBuilder::new)
//! * [platforms to simulate Cargo builds on](HakariBuilder::set_platforms)
//! * [the version of the Cargo resolver to use](HakariBuilder::set_resolver)
//! * [packages to be excluded during computation](HakariBuilder::add_traversal_excludes)
//! * [packages to be excluded from the final output](HakariBuilder::add_final_excludes)
//!
//! With the optional `cli-support` feature, `HakariBuilder` options can be
//! [read from](HakariBuilder::from_summary) or [written to](HakariBuilder::to_summary)
//! a file as TOML or some other format.
//!
//! ## 2. Computation
//!
//! Once a `HakariBuilder` is configured, its [`compute`](HakariBuilder::compute) method can be
//! called to create a `Hakari` instance. The algorithm runs in three steps:
//!
//! 1. Use guppy to [simulate a Cargo build](guppy::graph::cargo) for every workspace package and
//!    every given platform, with no features, default features and all features. Collect the
//!    results into
//!    [a map](internals::ComputedMap) indexed by every dependency and the different sets of
//!    features it was built with.
//! 2. Scan through the map to figure out which dependencies are built with two or more
//!    different feature sets, collecting them into an [output map](internals::OutputMap).
//! 3. If one assumes that the output map will be written out to the `workspace-hack` package
//!    through step 3 below, it is possible that it causes some extra packages to be built with a
//!    second feature set. Look for such packages, add them to the output map, and iterate until a
//!    fixpoint is reached and no new packages are built more than one way.
//!
//! This computation is done in a parallel fashion, using the [Rayon](rayon) library.
//!
//! The result of this computation is a [`Hakari`](Hakari) instance.
//!
//! ## 3. Serialization
//!
//! The last step is to serialize the contents of the output map into the `workspace-hack` package's
//! `Cargo.toml` file.
//!
//! 1. [`Hakari::read_toml`] reads an existing `Cargo.toml` file on disk. This file is
//!    *partially generated*:
//!
//!    ```toml
//!    [package]
//!    name = "workspace-hack"
//!    version = "0.1.0"
//!    # more options...
//!
//!    ### BEGIN HAKARI SECTION
//!    ...
//!    ### END HAKARI SECTION
//!    ```
//!
//!    The contents outside the `BEGIN HAKARI SECTION` and `END HAKARI SECTION` lines may be
//!    edited by hand. The contents within this section are automatically generated.
//!
//!    On success, a [`HakariCargoToml`](HakariCargoToml) is returned.
//!
//! 2. [`Hakari::to_toml_string`](Hakari::to_toml_string) returns the new contents of the
//!    automatically generated section.
//! 3. [`HakariCargoToml::write_to_file`](HakariCargoToml::write_to_file) writes out the contents
//!    to disk.
//!
//! `HakariCargoToml` also supports serializing contents to memory and producing diffs.
//!
//! # Future work
//!
//! `hakari` is still missing a few features:
//!
//! * Simulating cross-compilations
//! * Omitting some packages on some environments
//! * Only including a subset of packages in the final result (e.g. unifying core packages like
//!   `syn` but not any others)
//! * Support for alternate registries (depends on
//!   [Cargo issue #9052](https://github.com/rust-lang/cargo/issues/9052))
//!
//! These features will be added as time permits.

mod cargo_toml;
#[cfg(feature = "cli-support")]
pub mod cli_ops;
mod hakari;
#[cfg(feature = "proptest1")]
mod proptest_helpers;
#[cfg(feature = "cli-support")]
pub mod summaries;
mod toml_out;
pub mod verify;

pub use crate::{
    cargo_toml::*,
    hakari::{Hakari, HakariBuilder, UnifyTargetHost},
    toml_out::*,
};

pub mod internals {
    //! Access to internal Hakari data structures.
    //!
    //! These are provided in case some post-processing needs to be done.

    pub use crate::hakari::{
        ComputedInnerMap, ComputedInnerValue, ComputedMap, ComputedValue, OutputKey, OutputMap,
    };
}

/// Re-export diffy.
pub use diffy;
