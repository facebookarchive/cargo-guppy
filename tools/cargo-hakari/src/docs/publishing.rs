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
//! `cargo hakari` provides three ways to handle this. **For most users publishing to crates.io,
//! [method B] is the easiest**.
//!
//! [method B]: #b-target-the-workspace-hack-crate-already-on-cratesio
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
//! # B. Target the "workspace-hack" crate already on crates.io
//!
//! Methods B and C preserve workspace-hack dependencies in `Cargo.toml`s by targeting a stub
//! crate on the registry. The crates.io registry already contains an empty package called
//! [workspace-hack](https://crates.io/crates/workspace-hack), meant just for this.
//!
//! Starting from cargo-hakari 0.9.9, `cargo hakari init`'s default configuration makes steps 1 and
//! 2 unnecessary, making this method zero-setup.
//!
//! However, if the workspace-hack crate was initialized by an older version of cargo-hakari,
//! perform the following actions.
//!
//! ## 1. Ensure the local crate is called "workspace-hack"
//!
//! If your crate has a different name, rename it to `"workspace-hack"`.
//!
//! > **TIP:** On Unix platforms, to rename `my-workspace-hack` to `workspace-hack` in other
//! > `Cargo.toml` files: run this from the root of the workspace:
//! >
//! > ```sh
//! > git ls-files | grep Cargo.toml | xargs perl -p -i -e 's/^my-workspace-hack = /workspace-hack = /'
//! > ```
//! >
//! > If not in the context of a Git repository, run:
//! >
//! > ```sh
//! > find . -name Cargo.toml | xargs perl -p -i -e 's/^my-workspace-hack = /workspace-hack = /'`
//! > ```
//!
//! Remember to update `.config/hakari.toml` (or `.guppy/hakari.toml`) with the new name.
//!
//! ## 2. Ensure `dep-format-version = "2"` is set in `.config/hakari.toml`
//!
//! `dep-format-version = "2"` adds the `version` field to the `workspace-hack = ...` lines in other
//! `Cargo.toml` files. `cargo publish` uses the `version` field to recognize published
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
//! ## C. Publish your own workspace-hack crate to the registry
//!
//! If your crates need to be published to a different registry, or you wish to publish your own
//! version of the workspace-hack, follow these instructions.
//!
//! ## 1. Give the workspace-hack a unique name
//!
//! If your crate has a name that is already taken up on the registry, give it a unique name.
//!
//! ## 2. Ensure `dep-format-version = "2"` is set in `.config/hakari.toml`
//!
//! See [Method B] above for more about this.
//!
//! [Method B]: #2-ensure-dep-format-version--2-is-set-in-confighakaritoml
//!
//! ## 3. Set options in the workspace-hack's `Cargo.toml`
//!
//! In the workspace-hack's `Cargo.toml` file, set the `package.publish` option to anything other
//! than `false`. This enables publication of the workspace-hack crate.
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
//! Run `cargo hakari disable` to disable the workspace-hack.
//!
//! ## 5. Publish the stub workspace-hack crate
//!
//! If the workspace-hack crate has been renamed to `my-workspace-hack`, run `cargo publish -p
//! my-workspace-hack --allow-dirty` to publish the crate to `crates.io`. For other registries, use
//! the `--registry` flag.
//!
//! ## 6. Re-enable the workspace-hack crate
//!
//! Run `cargo hakari generate` to restore the workspace-hack's contents. You can also use your
//! source control system's commands to do so, such as with `git restore`.
