// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! About workspace-hack crates, how `cargo hakari` manages them, and how much faster they make builds.
//!
//! # What are workspace-hack crates?
//!
//! Let's say you have a Rust crate `my-crate` with two dependencies:
//!
//! ```toml
//! # my-crate/Cargo.toml
//! [dependencies]
//! foo = "1.0"
//! bar = "2.0"
//! ```
//!
//! Let's say that `foo` and `bar` both depend on `baz`:
//!
//! ```toml
//! # foo-1.0/Cargo.toml
//! [dependencies]
//! baz = { version = "1", features = ["a", "b"] }
//!
//! # bar-2.0/Cargo.toml
//! [dependencies]
//! baz = { version = "1", features = ["b", "c"] }
//! ```
//!
//! What features is `baz` built with?
//!
//! One way to resolve this question might be to build `baz` twice with each requested set of
//! features. But this is likely to cause a combinatorial explosion of crates to build, so Cargo
//! doesn't do that. Instead,
//! [Cargo builds `baz` once](https://doc.rust-lang.org/nightly/cargo/reference/features.html?highlight=feature#feature-unification)
//! with the *union* of the features enabled for the package: `[a, b, c]`.
//!
//! ---
//!
//! **NOTE:** This description elides some details around unifying build and dev-dependencies: for
//! more about this, see the documentation for guppy's
//! [`CargoResolverVersion`](guppy::graph::cargo::CargoResolverVersion).
//!
//! ---
//!
//! Now let's say you're in a workspace, with a second crate `your-crate`:
//!
//! ```toml
//! # your-crate/Cargo.toml
//! [dependencies]
//! baz = { version = "1", features = ["c", "d"] }
//! ```
//!
//! In this situation:
//!
//! | if you build                                 | `baz` is built with |
//! | -------------------------------------------- | ------------------- |
//! | just `my-crate`                              | `a, b, c`           |
//! | just `your-crate`                            | `c, d`              |
//! | `my-crate` and `your-crate` at the same time | `a, b, c, d`        |
//!
//! Even in this simplified scenario, there are three separate ways to build `baz`. For a dependency
//! like [`syn`](https://crates.io/crates/syn) that has
//! [many optional features](https://github.com/dtolnay/syn#optional-features),
//! large workspaces end up with a very large number of possible build configurations.
//!
//! Even worse, the feature set of a package affects everything that depends on it, so `syn`
//! being built with a slightly different feature set than before would cause *every package that
//! directly or transitively depends on `syn` to be rebuilt. For large workspaces, this can result
//! a lot of wasted build time.
//!
//! ---
//!
//! To avoid this problem, many large workspaces contain a `workspace-hack` crate. The
//! purpose of this package is to ensure that dependencies like `syn` are always built with the same
//! feature set no matter which workspace packages are currently being built. This is done by:
//! 1. adding dependencies like `syn` to `workspace-hack` with the full feature set required by any
//!   package in the workspace
//! 2. adding `workspace-hack` as a dependency of every crate in the repository.
//!
//! Some examples of `workspace-hack` packages:
//!
//! * Rust's [`rustc-workspace-hack`](https://github.com/rust-lang/rust/blob/0bfc45aa859b94cedeffcbd949f9aaad9f3ac8d8/src/tools/rustc-workspace-hack/Cargo.toml)
//! * Firefox's [`mozilla-central-workspace-hack`](https://hg.mozilla.org/mozilla-central/file/cf6956a5ec8e21896736f96237b1476c9d0aaf45/build/workspace-hack/Cargo.toml)
//! * Diem's [`diem-workspace-hack`](https://github.com/diem/diem/blob/91578fec8d575294b47b3ee7af691fd9dc6eb240/common/workspace-hack/Cargo.toml)
//!
//! These packages have historically been maintained by hand, on a best-effort basis.
//!
//! # What can hakari do?
//!
//! Maintaining workspace-hack packages manually can result in:
//! * Missing crates
//! * Missing feature lists for crates
//! * Outdated feature lists for crates
//!
//! All of these can result in longer than optimal build times.
//!
//! `cargo hakari` can automate the maintenance of these packages, greatly reducing the amount of
//! time and effort it takes to maintain these packages.
//!
//! # How does hakari work?
//!
//! `cargo hakari` uses [guppy]'s Cargo build simulations to determine the full set of features
//! that can be built for each package. It then looks for
//!
//! For more details about the algorithm, see the documentation for the [`hakari`] library.
//!
//! # How much faster do builds get?
//!
//! The amount to which builds get faster depends on the size of the repository. In general, the
//! benefit grows super-linearly with the size of the workspace and the number of crates in it.
//!
//! On moderately large workspaces with several hundred third-party dependencies, a cumulative
//! performance benefit of 20-25% has been seen. Individual commands can be anywhere from 10%
//! to 95+% faster. `cargo check` often benefits more than `cargo build` because expensive
//! linker invocations aren't a factor.
//!
//! ## Performance metrics
//!
//! All measurements were taken on the following system:
//!
//! * **Processor:** AMD Ryzen 9 3900X processor (12 cores, 24 threads)
//! * **Memory:** 64GB
//! * **Operating system:** [Pop!_OS 20.10](https://pop.system76.com/), running Linux kernel 5.13
//! * **Filesystem:** btrfs
//!
//! ---
//!
//! On the [Diem repository](https://github.com/diem/diem/), at revision 6fa1c8c0, with the following
//! `cargo build` commands in sequence:
//!
//! | Command                               | Before (s) | After (s) | Change   | Notes                                        |
//! |---------------------------------------|-----------:|----------:|---------:|----------------------------------------------|
//! | `-p move-lang`                        | 35.56      | 53.06     | 49.21%   | First command has to build more dependencies |
//! | `-p move-lang --all-targets`          | 46.64      | 25.45     | -45.44%  |                                              |
//! | `-p move-vm-types`                    | 10.56      | 0.29      | -97.24%  | This didn't have to build anything           |
//! | `-p network`                          | 19.16      | 14.10     | -26.42%  |                                              |
//! | `-p network --all-features`           | 21.59      | 18.20     | -15.70%  |                                              |
//! | `-p storage-interface`                | 7.04       | 2.97      | -57.83%  |                                              |
//! | `-p storage-interface --all-features` | 12.78      | 1.15      | -91.03%  |                                              |
//! | `-p diem-node`                        | 102.32     | 84.65     | -17.27%  | This command built a large C++ dependency    |
//! | `-p backup-cli`                       | 52.47      | 33.26     | -36.61%  | Linked several binaries                      |
//! | **Total**                             | 308.12     | 233.12    | -24.34%  |                                              |
//!
//! With the following `cargo check` commands in sequence:
//!
//! | Command                               | Before (s) | After (s) | Change  | Notes                                         |
//! |---------------------------------------|-----------:|----------:|--------:|-----------------------------------------------|
//! | `-p move-lang`                        | 16.04      | 36.55     | 127.83% | First command has to build more dependencies  |
//! | `-p move-lang --all-targets`          | 26.73      | 13.22     | -50.56% |                                               |
//! | `-p move-vm-types`                    | 9.41       | 0.29      | -96.91% | This didn't have to build anything            |
//! | `-p network`                          | 12.41      | 9.43      | -24.01% |                                               |
//! | `-p network --all-features`           | 15.12      | 11.54     | -23.69% |                                               |
//! | `-p storage-interface`                | 5.33       | 1.65      | -68.98% |                                               |
//! | `-p storage-interface --all-features` | 8.22       | 1.02      | -87.59% |                                               |
//! | `-p diem-node`                        | 56.60      | 51.29     | -9.38%  | This command built two large C++ dependencies |
//! | `-p backup-cli`                       | 13.57      | 5.51      | -59.40% |                                               |
//! | **Total**                             | 163.44     | 130.50    | -20.15% |                                               |//!
//! ---
//!
//! On the much smaller [cargo-guppy repository](https://github.com/facebookincubator/cargo-guppy),
//! at revision 65e8c8d7, with the following `cargo build` commands in sequence:
//!
//! | Command                    | Before (s) | After (s) | Change  | Notes                                        |
//! |----------------------------|-----------:|----------:|--------:|----------------------------------------------|
//! | `-p guppy`                 | 11.77      | 13.48     | 14.53%  | First command has to build more dependencies |
//! | `-p guppy --all-features`  | 9.83       | 9.72      | -1.12%  |                                              |
//! | `-p hakari`                | 6.03       | 3.75      | -37.94% |                                              |
//! | `-p hakari --all-features` | 10.78      | 10.28     | -4.68%  |                                              |
//! | `-p determinator`          | 4.60       | 3.90      | -15.22% |                                              |
//! | `-p cargo-hakari`          | 17.72      | 7.22      | -59.26% |                                              |
//! | **Total**                  | 60.73      | 48.34     | -20.41% |                                              |
