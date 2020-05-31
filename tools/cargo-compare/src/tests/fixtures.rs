// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::common::GuppyCargoCommon;
use guppy::graph::PackageGraph;
use guppy_cmdlib::CargoMetadataOptions;
use once_cell::sync::Lazy;
use proptest::prelude::*;
use std::env;
use std::path::Path;

// ---
// Paths to fixtures, relative to the cargo-compare directory (the one with Cargo.toml)
// ---
pub(super) static INSIDE_OUTSIDE_WORKSPACE: &str =
    "../../fixtures/workspace/inside-outside/workspace";
pub(super) static CARGO_GUPPY_WORKSPACE: &str = ".";

#[derive(Debug)]
pub struct Fixture {
    metadata_opts: CargoMetadataOptions,
    graph: PackageGraph,
}

macro_rules! define_fixture {
    ($name: ident, $path: ident) => {
        pub(crate) fn $name() -> &'static Fixture {
            static FIXTURE: Lazy<Fixture> = Lazy::new(|| Fixture::new($path));
            &*FIXTURE
        }
    };
}

static CARGO_MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

impl Fixture {
    pub fn new(workspace_dir: &str) -> Self {
        // Assume that the workspace is relative to `CARGO_MANIFEST_DIR`.
        let workspace_dir = Path::new(CARGO_MANIFEST_DIR).join(workspace_dir);
        if !workspace_dir.is_dir() {
            panic!(
                "workspace_dir {} is not a directory",
                workspace_dir.display()
            );
        }
        let metadata_opts = CargoMetadataOptions {
            manifest_path: Some(workspace_dir.join("Cargo.toml")),
        };
        let graph = metadata_opts
            .make_command()
            .build_graph()
            .expect("constructing package graph worked");

        Self {
            metadata_opts,
            graph,
        }
    }

    // ---
    // Fixtures
    // ---

    define_fixture!(inside_outside, INSIDE_OUTSIDE_WORKSPACE);
    define_fixture!(cargo_guppy, CARGO_GUPPY_WORKSPACE);

    // ---

    pub fn graph(&self) -> &PackageGraph {
        &self.graph
    }

    /// Returns the number of proptest iterations that should be run for this fixture.
    pub fn num_proptests(&self) -> u32 {
        // Large graphs (like cargo-guppy's) can only really do a tiny number of proptests
        // reasonably in debug mode. It would be cool to figure out a way to speed it up (release
        // mode works -- also maybe through parallelization?)
        static PROPTEST_MULTIPLIER: Lazy<u32> =
            Lazy::new(|| match env::var("PROPTEST_MULTIPLIER") {
                Ok(multiplier) => multiplier
                    .parse()
                    .expect("PROPTEST_MULTIPLIER is a valid u32"),
                Err(_) => 2,
            });
        if self.graph.package_count() > 100 {
            *PROPTEST_MULTIPLIER
        } else {
            *PROPTEST_MULTIPLIER * 4
        }
    }

    pub fn common_strategy<'a>(&'a self) -> impl Strategy<Value = GuppyCargoCommon> + 'a {
        GuppyCargoCommon::strategy(&self.metadata_opts, self.graph())
    }
}
