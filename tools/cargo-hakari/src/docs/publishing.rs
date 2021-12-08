// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Publishing a package to `crates.io` or other registries.
//!
//! *This section can be ignored if your workspace doesn't publish any crates to registries.*
//!
//! Many projects using `cargo hakari` may wish to publish their crates to `crates.io` or other
//! registries. However, if you attempt to publish a crate from a Hakari-managed workspace,
//! `cargo publish` may reject it for containing the local-only workspace-hack dependency.
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
//! # B. Publish your own workspace-hack crate to the registry
//!
//! This method preserves workspace-hack dependencies in `Cargo.toml`s by targeting a stub
//! crate on the registry.
//!
//! ## 1. Ensure the local crate is unique on the registry
//!
//! Rename it to something unique if necessary.
//!
//! > **TIP:** On Unix platforms, to rename `workspace-hack` to `my-workspace-hack` in other
//! > `Cargo.toml` files: run this from the root of the workspace:
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
//! Remember to update `.config/hakari.toml` (or `.guppy/hakari.toml`) with the new name.
//!
//! The rest of this section assumes the crate is called `my-workspace-hack`.
//!
//! ## 2. Ensure `dep-format-version = "2"` is set in `.config/hakari.toml`
//!
//! `dep-format-version = "2"` adds the `version` field to the `my-workspace-hack = ...` lines in
//! other `Cargo.toml` files. `cargo publish` uses the `version` field to recognize published
//! dependencies.
//!
//! This option is new in cargo-hakari 0.9.8. Configuration files created by older versions of
//! cargo-hakari may not have this option set.
//!
//! Ensure that this option is present in `.config/hakari.toml`:
//!
//! ```toml
//! dep-format-version = "2"
//! ```
//!
//! Then run `cargo hakari manage-deps` to update the `workspace-hack = ...` lines.
//!
//! ---
//!
//! After performing the above actions, simply run `cargo publish` as usual to publish the crate.
//!
//! ## 3. Set options in the workspace-hack's `Cargo.toml`
//!
//! In `my-workspace-hack`'s `Cargo.toml` file, set the `package.publish` option to anything other
//! than `false`. This enables its publication.
//!
//! ```toml
//! [package]
//! publish = true  # to allow publishing to any registry
//! ## or
//! publish = ["crates-io"]  # to allow publishing to crates.io only
//! ```
//!
//! While you're here, you may also wish to set other options like `repository` or `homepage`.
//!
//! ## 4. Temporarily disable the workspace-hack crate
//!
//! **This step is really important.** Not doing it will cause the full dependency set in the
//! workspace-hack to be published, which is not what you want.
//!
//! Run `cargo hakari disable` to disable the workspace-hack crate`.
//!
//! ## 5. Publish the stub workspace-hack crate
//!
//! Run `cargo publish -p my-workspace-hack --allow-dirty` to publish the crate to `crates.io`.
//! For other registries, use the `--registry` flag.
//!
//! ## 6. Re-enable the workspace-hack crate
//!
//! Run `cargo hakari generate` to restore the workspace-hack's contents. You can also use your
//! source control system's commands to do so, such as with `git restore`.
