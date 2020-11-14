// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

#![warn(missing_docs)]

//! Figure out what packages in a Rust workspace changed between two commits.
//!
//! A typical continuous integration system runs every test on every pull request or proposed
//! change. In large monorepos, most proposed changes have no effect on most packages. A
//! *target determinator* figures out, given a proposed change, which packages may have had changes
//! to them.
//!
//! The determinator is desiged to be used in the
//! [Libra Core workspace](https://github.com/libra/libra), which is one such monorepo.
//!
//! # Optional features
//!
//! * `serde1`: Support loading custom determinator rules from a file.
//!
//! # How it works
//!
//! Broadly speaking, a change to a Rust package can come from one of two sources:
//! * The source code or `Cargo.toml` of the package changing.
//! * A change to a dependency, which can happen in one of three ways:
//!   1. The source code of a workspace dependency changing.
//!   2. A version bump to a third-party dependency.
//!   3. The feature set of a dependency changing.
//!
//! The determinator pieces together data from both of these sources, along with information
//! provided by [guppy](https://docs.rs/guppy), to figure out which packages need to be re-tested.
//!
//! ## File changes
//!
//! The determinator expects to be passed in a list of file changes between two revisions. For each
//! file passed in:
//! * The determinator looks for the package nearest to the file and marks it as changed.
//! * If the file is outside a package, the determinator ignores the file by default.
//!
//! The list of file changes can be obtained from a source control system such as Git. `Paths0`,
//! available in this crate, can help.
//!
//! These simple rules may need to be customized for particular scenarios (e.g. if a special file in
//! the root of the workspace changes, mark the entire workspace changed). For such situations, the
//! determinator has support for custom rules. See the documentation for the `rules` module for
//! more.
//!
//! ## Dependency changes
//!
//! The determinator uses `guppy` to run Cargo build simulations on every package in the workspace.
//! For each package, the determinator figures out whether any of its dependencies (including
//! feature sets) have changed. These simulations are done with:
//! * dev-dependencies enabled (by default; this can be customized)
//! * both the host and target platforms set to the current platform (by default; this can be
//!   customized)
//! * three sets of features for each package:
//!   * no features enabled
//!   * default features
//!   * all features enabled
//!
//! If any of these simulated builds indicates that a workspace package has had any dependency
//! changes through:
//! * a file change, or
//! * a third-party dependency change (version or feature set change)
//!
//! then it is marked changed.
//!
//! # Customizing behavior
//!
//! The standard rules followed by the determinator may need to be tweaked in some situations:
//! * Some files should be ignored.
//! * If some files or packages change, a full test run may be necessary.
//! * "Virtual dependencies" that Cargo isn't aware of may need to be inserted.
//!
//! For these situations, the determinator allows for custom *rules* to be specified. For more,
//! see the documentation for the `rules` module.
//!
//! # Limitations
//!
//! The determinator may not be able to figure out third-party changes outside the workspace if they
//! aren't accompanied with a version bump. This is not an issue for third-party crates retrieved
//! from `crates.io` or a Git repository, but may be one for third-party dependencies on
//! the file system. A future TODO is to add support for assuming that a third-party package has
//! changed.
//!
//! The determinator is also unaware of changes to the build environment---in those cases, a full
//! build may have to be forced from outside the determinator. In general, it is recommended that
//! the build environment be checked into the repository (e.g. through [GitHub Actions workflow
//! files](https://docs.github.com/en/free-pro-team@latest/actions/reference/workflow-syntax-for-github-actions))
//! and a full build be forced if any of those files change.
//!
//! # Alternatives
//!
//! One way to look at the determinator is through the lens of *caching*: test results can be
//! cached, and changes can be analyzed to
//! [invalidate cache results](https://martinfowler.com/bliki/TwoHardThings.html).
//!
//! There are a number of other caching systems in existence, such as:
//! * [Mozilla's `sccache`](https://github.com/mozilla/sccache) and other "bottom-up" caching build
//!   systems.
//! * [Bazel](https://bazel.build/), [Buck](https://buck.build/) and other "top-down" hash-based
//!   caching build systems.
//!
//! While these systems are great, they may not always be practical (in particular, `sccache`
//! requires paths to be exact across machines, and Bazel and Buck have stringent requirements
//! around the environment not affecting build results.) These systems are also geared towards
//! builds, which tend to be more hermetic than test results.
//!
//! However, it is quite likely that in many cases one of these other systems may provide better
//! results. In the [Libra Core workspace](https://github.com/libra/libra), the current plan is to
//! perform builds with `sccache`, and to use this determinator to figure out which tests to run.
//! This may change as we learn more about how each of these systems behave in practice.
//!
//! # Inspirations
//!
//! This determinator is inspired by the one used in Facebook's main source repository.

mod determinator;
pub mod errors;
mod paths0;
pub mod rules;

pub use crate::determinator::*;
pub use crate::paths0::*;
