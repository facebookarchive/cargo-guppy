// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Configuration for `cargo hakari`.
//!
//! # Configuration file location
//!
//! The default config path for cargo-hakari versions 0.9.8 or above is `.config/hakari.toml`,
//! relative to the root of the workspace. Previous versions used `.guppy/hakari.toml`, which
//! continues to be supported as a fallback.
//!
//! # Common options
//!
//! ## hakari-package
//!
//! The name of the hakari-managed crate in the workspace. Must be specified. For example:
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
//! Defaults to "1", but `.config/hakari.toml` files created by `cargo hakari init` set it to "2".
//!
//! ```toml
//! resolver = "2"
//! ```
//!
//! ## dep-format-version
//!
//! The version of `workspace-hack = ...` lines in other `Cargo.toml` files to use.
//!
//! Possible values:
//! * *"1"*: `workspace-hack = { path = ...}`. (Note the lack of a trailing space.)
//! * *"2"*: `workspace-hack = { version = "0.1", path = ... }`. This is required for the advanced
//!   setup documented in the [Publishing](crate::publishing) section.
//!
//! Defaults to "1", but starting `cargo hakari 0.9.8`, `.config/hakari.toml` files created by
//! `cargo hakari init` set it to "2".
//!
//! ```toml
//! dep-format-version = "2"
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
//! Defaults to an empty list.
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
//! Defaults to an empty set.
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
//! Defaults to an empty set.
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
//! ## registries
//!
//! Alternate registries,
//! [in the same format](https://doc.rust-lang.org/cargo/reference/registries.html) as
//! `.cargo/config.toml`.
//!
//! This is a temporary workaround until [Cargo issue #9052](https://github.com/rust-lang/cargo/issues/9052)
//! is resolved.
//!
//! Defaults to an empty set.
//!
//! ```toml
//! [registries]
//! my-registry = { index = "https://my-intranet:8080/git/index" }
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
//! Defaults to false.
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
//! If the same dependency is built on both the target and host platforms, this option controls
//! whether and how they should be unified.
//!
//! The possible options are `"none"`, `"auto"`, `"unify-if-both"`, and
//! `"replicate-target-on-host"`. For more about these options, see the documentation for
//! [`UnifyTargetHost`](hakari::UnifyTargetHost).
//!
//! Defaults to `"auto"`.
//!
//! ```toml
//! unify-target-host = "replicate-target-on-host"
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
//! Defaults to false.
//!
//! ```toml
//! output-single-feature = true
//! ```
