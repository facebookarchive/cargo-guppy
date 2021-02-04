// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

#![warn(missing_docs)]

//! Figure out what packages in a Rust workspace changed between two commits.
//!
//! A typical continuous integration system runs every build and every test on every pull request or
//! proposed change. In large monorepos, most proposed changes have no effect on most packages. A
//! *target determinator* decides, given a proposed change, which packages may have had changes
//! to them.
//!
//! The determinator is desiged to be used in the
//! [Diem Core workspace](https://github.com/diem/diem), which is one such monorepo.
//!
//! # Examples
//!
//! ```rust
//! use determinator::{Determinator, rules::DeterminatorRules};
//! use guppy::{CargoMetadata, graph::DependencyDirection};
//! use std::path::Path;
//!
//! // guppy accepts `cargo metadata` JSON output. Use a pre-existing fixture for these examples.
//! let old_metadata = CargoMetadata::parse_json(include_str!("../../../fixtures/guppy/metadata_guppy_78cb7e8.json")).unwrap();
//! let old = old_metadata.build_graph().unwrap();
//! let new_metadata = CargoMetadata::parse_json(include_str!("../../../fixtures/guppy/metadata_guppy_869476c.json")).unwrap();
//! let new = new_metadata.build_graph().unwrap();
//!
//! let mut determinator = Determinator::new(&old, &new);
//!
//! // The determinator supports custom rules read from a TOML file.
//! let rules = DeterminatorRules::parse(include_str!("../../../fixtures/guppy/path-rules.toml")).unwrap();
//! determinator.set_rules(&rules).unwrap();
//!
//! // The determinator expects a list of changed files to be passed in.
//! determinator.add_changed_paths(vec![Path::new("guppy/src/lib.rs"), Path::new("tools/determinator/README.md")]);
//!
//! let determinator_set = determinator.compute();
//! // determinator_set.affected_set contains the workspace packages directly or indirectly affected
//! // by the change.
//! for package in determinator_set.affected_set.packages(DependencyDirection::Forward) {
//!     println!("affected: {}", package.name());
//! }
//! ```
//!
//! # Platform support
//!
//! * **Unix platforms**: The determinator works and is supported.
//! * **Windows**: experimental support. There may still be bugs around path normalization: please
//!   [report them](https://github.com/facebookincubator/cargo-guppy/issues/new)!
//!
//! # How it works
//!
//! A Rust package can behave differently if one or more of the following change:
//! * The source code or `Cargo.toml` of the package.
//! * A dependency.
//! * The build or test environment.
//!
//! The determinator gathers data from several sources, and processes it through
//! [guppy](https://docs.rs/guppy), to figure out which packages need to be re-tested.
//!
//! ## File changes
//!
//! The determinator takes as input a list of file changes between two revisions. For each
//! file provided:
//! * The determinator looks for the package nearest to the file and marks it as changed.
//! * If the file is outside a package, the determinator assumes that everything needs to be
//!   rebuilt.
//!
//! The list of file changes can be obtained from a source control system such as Git. This crate
//! provides a helper which simplifies the process of enumerating file lists while handling some
//! gnarly edge cases. For more information, see the documentation for [`Paths0`](crate::Paths0).
//!
//! These simple rules may need to be customized for particular scenarios (e.g. to ignore certain
//! files, or mark a package changed if a file outside of it changes). For those situations, the
//! determinator lets you specify *custom rules*. See the
//! [Customizing behavior](#customizing-behavior) section below for more.
//!
//! ## Dependency changes
//!
//! A dependency is assumed to have changed if one or more of the following change:
//!
//! * For a workspace dependency, its source code.
//! * For a third-party dependency, its version or feature set.
//! * Something in the environment that it depends on.
//!
//! The determinator runs Cargo build simulations on every package in the workspace. For each
//! package, the determinator figures out whether any of its dependencies (including feature sets)
//! have changed. These simulations are done with:
//! * dev-dependencies enabled (by default; this can be customized)
//! * both the host and target platforms set to the current platform (by default; this can be
//!   customized)
//! * three sets of features for each package:
//!   * no features enabled
//!   * default features
//!   * all features enabled
//!
//! If any of these simulated builds indicates that a workspace package has had any dependency
//! changes, then it is marked changed.
//!
//! ## Environment changes
//!
//! The *environment* of a build or test run is anything not part of the source code that may
//! influence it. This includes but is not limited to:
//!
//! * the version of the Rust compiler used
//! * system libraries that a crate depends on
//! * environment variables that a crate depends on
//! * external services that a test depends on
//!
//! **By default, the determinator assumes that the environment stays the same between runs.**
//!
//! To represent changes to the environment, you may need to find ways to represent those changes
//! as files checked into the repository, and add [custom rules](#customizing-behavior) for them.
//! For example:
//!
//! * Use a [`rust-toolchain` file](https://doc.rust-lang.org/edition-guide/rust-2018/rustup-for-managing-rust-versions.html#managing-versions)
//!   to represent the version of the Rust compiler. There is a default rule which causes a full
//!   run if `rust-toolchain` changes.
//! * Record all environment variables in CI configuration files, such as [GitHub Actions workflow
//!   files](https://docs.github.com/en/free-pro-team@latest/actions/reference/workflow-syntax-for-github-actions),
//!   and add a custom rule to do a full run if any of those files change.
//! * As far as possible, make tests hermetic and not reach out to the network. If you only have a
//!   few tests that make network calls, run them unconditionally.
//!
//! # Customizing behavior
//!
//! The standard rules followed by the determinator may need to be tweaked in some situations:
//! * Some files should be ignored.
//! * If some files or packages change, a full test run may be necessary.
//! * *Virtual dependencies* that Cargo isn't aware of may need to be inserted.
//!
//! For these situations, the determinator allows for custom *rules* to be specified. The
//! determinator also ships with
//! [a default set of rules](crate::rules::DeterminatorRules::DEFAULT_RULES_TOML) for common files
//! like `.gitignore` and `rust-toolchain`.
//!
//! For more about custom rules, see the documentation for the [`rules` module](crate::rules).
//!
//! # Limitations
//!
//! While the determinator can bring significant benefits to CI and local workflows, its model is
//! quite different from Cargo's. **Please understand these limitations before using the
//! determinator for your project.**
//!
//! For best results, consider doing occasional full runs in addition to determinator-based runs.
//! You may wish to configure your CI system to use the determinator for pull-requests, and also
//! schedule full runs every few hours on the main branch in case the determinator misses something.
//!
//! ## Build scripts and include/exclude instructions
//!
//! **The determinator cannot run [build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html).**
//! The standard Cargo method for declaring a dependency on a file or environment variable is to
//! output `rerun-if-changed` or `rerun-if-env-changed` instructions in build scripts. These
//! instructions must be duplicated through custom rules.
//!
//! **The determinator doesn't track the [`include` and `exclude` fields in `Cargo.toml`][include].**
//! This is because the determinator's view of what's changed doesn't always align with these fields.
//! For example, packages typically include `README` files, but the determinator has a default rule
//! to ignore them.
//!
//! If a package includes a file outside of it, either move it into the package (recommended) or
//! add a custom rule for it. Exclusions may be duplicated as custom rules that cause those files
//! to be ignored.
//!
//! [include]: https://doc.rust-lang.org/cargo/reference/manifest.html#the-exclude-and-include-fields
//!
//! ## Path dependencies outside the workspace
//!
//! **The determinator may not be able to figure out changes to path dependencies outside the
//! workspace.** The determinator relies on metadata to figure out whether a non-workspace
//! dependency has changed. The metadata includes:
//! * the version number
//! * the source, such as `crates.io` or a revision in a Git repository
//!
//! This approach works for dependencies on `crates.io` or other package repositories, because a
//! change to their source code necessarily requires a version change.
//!
//! This approach also works for [Git
//! dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#specifying-dependencies-from-git-repositories).
//! It even works for Git dependencies that aren't pinned to an exact revision in `Cargo.toml`,
//! because `Cargo.lock` records exact revisions. For example:
//!
//! ```toml
//! # Specifying this in Cargo.toml...
//! [dependencies]
//! rand = { git = "https://github.com/rust-random/rand", branch = "master" }
//!
//! # ...results in Cargo.lock with:
//! [[package]]
//! name = "rand"
//! version = "0.7.4"
//! source = "git+https://github.com/rust-random/rand?branch=master#50c34064c80762ddae11447adc6240f42a6bd266"
//! ```
//!
//! The hash at the end is the exact Git revision used, and a change to it is recognized by the
//! determinator.
//!
//! Where this scheme may not work is with path dependencies, because the files on disk can change
//! without a version bump. `cargo build` can recognize those changes because it compares mtimes of
//! files on disk, but the determinator cannot do that.
//!
//! This is not expected to be a problem for most projects that use workspaces. If
//! there's future demand, it would be possible to add support for changes to non-workspace path
//! dependencies if they're in the same repository.
//!
//! # Alternatives and tradeoffs
//!
//! One way to look at the determinator is as a kind of
//! [*cache invalidation*](https://martinfowler.com/bliki/TwoHardThings.html). Viewed through this
//! lens, the main purpose of a build or test system is to cache results, and invalidate those
//! caches based on certain parameters. When the determinator marks a package as changed, it
//! invalidates any cached results for that package.
//!
//! There are several other ways to design caching systems:
//! * The caching built into Cargo and other systems like GNU Make, which is based on file
//!   modification times.
//! * [Mozilla's `sccache`](https://github.com/mozilla/sccache) and other "bottom-up" hash-based
//!   caching build systems.
//! * [Bazel](https://bazel.build/), [Buck](https://buck.build/) and other "top-down" hash-based
//!   caching build systems.
//!
//! These other systems end up making different tradeoffs:
//! * Cargo can use build scripts to track file and environment changes over time. However, it
//!   relies on a previous build being done on the same machine. Also, as of Rust 1.48, there is no
//!   way to use Cargo caching for test results, only for builds.
//! * `sccache` [requires paths to be exact across machines][known-caveats], and is unable to cache
//!   [some kinds of Rust artifacts][rust-caveats]. Also, just like Cargo's caching, there is no way
//!   to use it for test results, only for builds.
//! * Bazel and Buck have stringent requirements around the environment not affecting build results.
//!   They're also not seamlessly integrated with Cargo.
//! * The determinator works for both builds and tests, but cannot track file and environment
//!   changes over time and must rely on custom rules. This scheme may produce both false negatives
//!   and false positives.
//!
//! [known-caveats]: https://github.com/mozilla/sccache#known-caveats
//! [rust-caveats]: https://github.com/mozilla/sccache/blob/master/docs/Rust.md
//!
//! While the determinator is geared towards test runs, it also works for builds. If you wish to
//! use the determinator for build runs, consider stacking it with another layer of caching:
//! * Use the determinator as a first pass to filter out packages that haven't changed.
//! * Then use a system like `sccache` to get hash-based caching for builds.
//!
//! # Inspirations
//!
//! This determinator is inspired by, and shares its name with, the target determinator used in
//! Facebook's main source repository.

mod determinator;
pub mod errors;
mod paths0;
pub mod rules;

pub use crate::{determinator::*, paths0::*};
