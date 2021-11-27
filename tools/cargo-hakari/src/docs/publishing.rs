// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Publishing a package to `crates.io` or other registries.
//!
//! *This section can be safely ignored if your workspace doesn't publish any crates to registries.*
//!
//! Many projects using `cargo hakari` may wish to publish their crates to `crates.io` or other
//! registries. However, these registries cannot work with local-only dependencies like
//! `workspace-hack`.
//!
//! `cargo hakari` provides two ways to handle this.
//!
//! # A. Temporarily remove the workspace-hack dependency before publishing
//!
//! Simply run:
//!
//! ```sh
//! cargo hakari publish -p <crate>
//! ```
//!
//! This command temporarily removes the dependency on the `workspace-hack` before publishing the
//! crate. The dependency will be re-added afterwards, unless the command is interrupted with
//! ctrl-C (in which case you can use `cargo hakari manage-deps` to finish the job.)
//!
//! This works out of the box. However, it has the downside of requiring `cargo hakari publish`.
//! If you don't have control over the commands run while publishing the package, it won't be
//! possible to use this method.
//!
//! # B. Publish a dummy workspace-hack crate to the registry
//!
//! In this method, you will publish an empty workspace-hack crate to the registry. This is a small
//! amount of **one-time** work.
//!
//! This method is used by [this repository](https://crates.io/crates/guppy-workspace-hack), as
//! well as [`rustc`](https://crates.io/crates/rustc-workspace-hack). It requires a few minutes of
//! work, but will work with all workflows.
//!
//! ## 1. Give the workspace-hack a unique name
//!
//! If your crate has a generic name like `workspace-hack`, rename it to something unique.
//!
//! For the rest of this example, the crate will be renamed to `my-workspace-hack`.
//!
//! > **TIP:** On Unix platforms, to rename `workspace-hack` in other `Cargo.toml` files: run
//! > this from the root of the workspace:
//! >
//! > ```sh
//! > git ls-files | grep Cargo.toml | xargs perl -p -i -e 's/^workspace-hack = /my-workspace-hack = /'
//! > ```
//! >
//! > If not in the context of a Git repository, run:
//! >
//! > ```sh
//! > find . -name Cargo.toml | xargs perl -p -i -e 's/^workspace-hack = /my-workspace-hack = /'`
//! > ```
//!
//! Remember to update `.config/hakari.toml` with the new package name.
//!
//! ## 2. Ensure `dep-format-version = "2"` is set in `.config/hakari.toml`
//!
//! If the config file was created with an older version of `cargo hakari`, it may not have this
//! option set. Add this line:
//!
//! ```toml
//! dep-format-version = "2"
//! ```
//!
//! Then run `cargo hakari manage-deps` to update the `workspace-hack = ...` lines.
//!
//! ## 3. Set options in the workspace-hack's `Cargo.toml`
//!
//! Set the `package.publish` option to anything other than `false`. This lets you publish the
//! workspace-hack crate to registries.
//!
//! ```toml
//! [package]
//! publish = true  # to allow publishing to any registry
//! # or
//! publish = ["crates-io"]  # to allow publishing to crates.io only
//! ```
//!
//! While you're here, you may also wish to set other options like `repository` or `homepage`.
//!
//! ## 4. Temporarily disable the workspace-hack
//!
//! **This step is really important.** Not doing it will cause the full workspace-hack to be
//! published, which is not what you want.
//!
//! Run `cargo hakari disable` to disable the workspace-hack.
//!
//! ## 5. Publish the dummy workspace-hack crate
//!
//! Run `cargo publish -p my-workspace-hack --allow-dirty` to publish the crate to `crates.io`.
//! For other registries, use the `--registry` flag.
//!
//! ## 6. Re-enable the workspace-hack
//!
//! Run `cargo hakari generate` to restore the workspace-hack's contents. You can also use your
//! source control system's commands to do so, such as with `git restore`.
