// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::common::GuppyCargoCommon;
use guppy::graph::{cargo::CargoResolverVersion, PackageGraph};
use guppy_cmdlib::CargoMetadataOptions;
use once_cell::sync::Lazy;
use proptest::prelude::*;
use std::{env, io::Write, path::Path};
use tempfile::TempDir;

// ---
// Paths to fixtures, relative to the cargo-compare directory (the one with Cargo.toml)
// ---
pub(super) static INSIDE_OUTSIDE_WORKSPACE: &str =
    "../../fixtures/workspace/inside-outside/workspace";
pub(super) static INSIDE_OUTSIDE_COPY_DIR: &str = "../../fixtures/workspace/inside-outside";
pub(super) static CARGO_GUPPY_WORKSPACE: &str = ".";

#[derive(Debug)]
pub struct Fixture {
    metadata_opts: CargoMetadataOptions,
    graph: PackageGraph,
    resolver: CargoResolverVersion,
    // Held on to to keep the temp dir around for the duration of the test.
    _temp_dir: Option<TempDir>,
}

macro_rules! define_fixture {
    (
        name => $name: ident,
        path => $path: ident,
        resolver => $resolver: expr,
        copy_dir => $copy_dir: expr,
    ) => {
        pub(crate) fn $name() -> &'static Fixture {
            static FIXTURE: Lazy<Fixture> = Lazy::new(|| Fixture::new($path, $resolver, $copy_dir));
            &*FIXTURE
        }
    };
}

static CARGO_MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

impl Fixture {
    pub fn new(
        workspace_dir: &str,
        resolver: CargoResolverVersion,
        copy_dir: Option<&str>,
    ) -> Self {
        // Assume that the workspace is relative to `CARGO_MANIFEST_DIR`.
        let orig_workspace_dir = Path::new(CARGO_MANIFEST_DIR).join(workspace_dir);

        let (temp_dir, workspace_dir) = if let Some(copy_dir) = copy_dir {
            // Create a temp dir and copy the files over.
            let temp_dir = tempfile::Builder::new()
                .prefix("cargo-compare")
                .tempdir()
                .expect("tempdir created");
            let copy_opts = fs_extra::dir::CopyOptions::new();

            let copy_from = Path::new(CARGO_MANIFEST_DIR).join(copy_dir);
            fs_extra::dir::copy(&copy_from, temp_dir.path(), &copy_opts)
                .expect("copying dir was successful");

            for entry in std::fs::read_dir(temp_dir.path()).unwrap() {
                let entry = entry.unwrap();
                println!("{:?}", entry);
            }

            // Grab the path to `Cargo.toml`. (fs_extra copies the directory into the tempdir so we
            // need to use the parent.)
            let relpath = pathdiff::diff_paths(&orig_workspace_dir, copy_from.parent().unwrap())
                .expect("both paths are absolute");
            let workspace_dir = temp_dir.path().join(&relpath);
            let workspace_dir = workspace_dir.canonicalize().unwrap_or_else(|err| {
                panic!(
                    "new workspace_dir {} canonicalized: {}",
                    workspace_dir.display(),
                    err,
                )
            });
            let workspace_manifest_path = workspace_dir.join("Cargo.toml");

            let mut open_opts = std::fs::OpenOptions::new();
            open_opts.append(true).write(true);
            let mut f = open_opts
                .open(&workspace_manifest_path)
                .expect("successfully opened Cargo.toml");
            let resolver_version = match resolver {
                CargoResolverVersion::V1 | CargoResolverVersion::V1Install => "1",
                CargoResolverVersion::V2 => "2",
                _ => panic!("unknown resolver {:?}", resolver),
            };
            writeln!(f, "resolver = \"{}\"", resolver_version).expect("file written successfully");

            (Some(temp_dir), workspace_dir)
        } else {
            (None, orig_workspace_dir)
        };

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
            resolver,
            _temp_dir: temp_dir,
        }
    }

    // ---
    // Fixtures
    // ---

    define_fixture! {
        name => inside_outside_v1,
        path => INSIDE_OUTSIDE_WORKSPACE,
        resolver => CargoResolverVersion::V1,
        copy_dir => Some(INSIDE_OUTSIDE_COPY_DIR),
    }
    define_fixture! {
        name => inside_outside_v2,
        path => INSIDE_OUTSIDE_WORKSPACE,
        resolver => CargoResolverVersion::V2,
        copy_dir => Some(INSIDE_OUTSIDE_COPY_DIR),
    }
    define_fixture! {
        name => cargo_guppy,
        path => CARGO_GUPPY_WORKSPACE,
        resolver => CargoResolverVersion::V2,
        copy_dir => None,
    }

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

    pub fn common_strategy(&self) -> impl Strategy<Value = GuppyCargoCommon> + '_ {
        GuppyCargoCommon::strategy(&self.metadata_opts, self.graph(), self.resolver)
    }
}
