// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Configuration for `cargo hakari`.
//!
//! Set these config options in `.guppy/hakari.toml` at the root of the workspace.
//!
//! # Common options
//!
//! ## hakari-package
//!
//! The name of the hakari-managed crate in the workspace. For example:
//!
//! ```toml
//! hakari-package = "my-workspace-hack"
//! ```
//!
//! ## resolver
//!
//! The version of the Cargo feature resolver to use. Version 2 is highly recommended.
//! For more, see this [Rust blog post](https://blog.rust-lang.org/2021/03/25/Rust-1.51.0.html#cargos-new-feature-resolver).
//!
//! ```toml
//! resolver = "2"
//! ```
//!
//! ## platforms
//!
//! Platforms to run specific queries on.
//!
//! By default, `cargo hakari` produces the minimal set of features that can be unified across
//! all possible platforms. However, in practice, most developers on a codebase use one of a
//! few platforms. `cargo hakari` can run specific queries for a few platforms, producing better
//! results for them.
//!
//! ```toml
//! ## Unify features on x86_64 Linux, Mac and Windows.
//! platforms = [
//!      "x86_64-unknown-linux-gnu",
//!      "x86_64-apple-darwin",
//!      "x86_64-pc-windows-msvc",
//! ]
//! ```
//!
//! ## traversal-excludes
//!
//! Crates to exclude while traversing the dependency graph.
//!
//! Packages specified in `traversal-excludes` will be omitted while searching for dependencies.
//! These packages will not be included in the final output. Any transitive dependencies of
//! these packages will not be included in the final result, unless those dependencices are reachable
//! from other crates.
//!
//! Workspace crates excluded from traversals will not depend on the workspace-hack crate, and
//! `cargo hakari manage-deps` will *remove* dependency edges rather than adding them.
//!
//! This is generally useful for crates that have mutually exclusive features, and that turn on
//! mutually exclusive features in their transitive dependencies.
//!
//! ```toml
//! [traversal-excludes]
//! workspace-members = ["my-crate", "my-other-crate"]
//! third-party = [
//!     ## Third-party crates accept semver ranges.
//!     { name = "mutually-exclusive-crate", version = "1.0" },
//!
//!     ## The version specifier can be skipped to include all versions of a crate.
//!     ## (Cryptography-related crates often use features to switch on different backends.)
//!     { name = "my-cryptography" },
//!
//!     ## Git and path dependencies can also be specified
//!     { name = "git-dependency", git = "https://github.com/my-org/git-dependency" },
//!     { name = "path-dependency", path = "../my/path/dependency" }
//! ]
//! ```
//!
//! ## final-excludes
//!
//! Crates to remove at the end of computation.
//!
//! Packages specified in `final-excludes` will be removed from the output at the very end. This
//! means that any transitive dependencies of theirs will still be included.
//!
//! Workspace crates excluded from the final output will not depend on the workspace-hack crate, and
//! `cargo hakari manage-deps` will *remove* dependency edges rather than adding them.
//!
//! This is generally useful for crates that have mutually exclusive features.
//!
//! This accepts configuration in the same format as `traversal-excludes` above.
//!
//! ```toml
//! [final-excludes]
//! workspace-members = ["my-crate", "your-crate"]
//! third-party = [
//!     ## The "fail" crate uses the "failpoints" feature to enable random errors at runtime.
//!     ## It is a good candidate for exclusion from the final output.
//!     { name = "fail" },
//!
//!     ## Version specifiers and git/path dependencies work similarly to traversal-excludes
//!     ## above.
//! ]
//! ```
//!
//! # Output options
//!
//! ## exact-versions
//!
//! By default, the workspace-hack crate's `Cargo.toml` file will contain a semver range. With
//! `exact-versions` turned on, the version currently in use will be output.
//!
//! This is most useful for situations where the `Cargo.lock` file is checked in, and if
//! version numbers are kept in sync across `Cargo.toml` and `Cargo.lock`. This includes some
//! configurations of [Dependabot](https://dependabot.com/).
//!
//! ```toml
//! exact-versions = true
//! ```
//!
//! # Advanced options
//!
//! ## unify-target-host
//!
//! Controls unification across target and host platforms.
//!
//! With version 2 of the feature resolver, if the same crate is included on both the target and
//! host platforms, it may be built in two different ways. This is not always desirable, and
//! `cargo hakari` can unify dependencies such that they're only built one way.
//!
//! The possible options are `"none"`, `"unify-on-both"`, and `"replicate-target-on-host"`
//! (default). For more about these options, see the documentation for
//! [`UnifyTargetHost`](hakari::UnifyTargetHost).
//!
//! ```toml
//! unify-target-host = "replicate-host-on-target"
//! ```
//!
//! ## output-single-feature
//!
//! By default, `cargo hakari` only outputs lines corresponding to third-party dependencies which
//! are built with at least two different sets of features. Setting this option to true will
//! cause `cargo hakari` to output lines corresponding to dependencies built with just one set
//! of features.
//!
//! This is generally not needed but may be useful in some situations.
//!
//! ```toml
//! output-single-feature = true
//! ```
