// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo hakari` is a command-line application to manage workspace-hack crates. Use it to speed up
//! local `cargo build` and `cargo check` commands by **15-95%**, and cumulatively by
//! **20-25% or more**.
//!
//! For an explanation of what workspace-hack packages are and how they make your builds faster, see
//! the [`about` module](https://docs.rs/cargo-hakari/latest/cargo_hakari/about).
//!
//! # Examples
//!
//! The `cargo-guppy` repository uses a workspace-hack crate managed by `cargo hakari`. [See the
//! generated `Cargo.toml`.](https://github.com/facebookincubator/cargo-guppy/blob/main/workspace-hack/Cargo.toml)
//!
//! # Platform support
//!
//! * **Unix platforms**: Hakari works and is supported.
//! * **Windows**: Hakari works and outputs file paths with forward slashes for
//!   consistency with Unix. CRLF line endings are not supported in the workspace-hack's
//!   `Cargo.toml`. Within Git repositories, `cargo hakari init` automatically does this for you.
//!   [Here's how to do it manually.](https://stackoverflow.com/a/10017566)
//!   (Pull requests to improve this are welcome.)
//!
//! # Installation
//!
//! All of the below commands take options that control their behavior.
//!
//! To install, run:
//!
//! ```sh
//! cargo install cargo-hakari
//! ```
//!
//! To update, run:
//!
//! ```sh
//! cargo install --force cargo-hakari
//! ```
//!
//! If `$HOME/.cargo/bin` is in your `PATH`, the `cargo hakari` command will be available.
//!
//! # Usage
//!
//! ## Getting started
//!
//! There are three steps you *must* take for `cargo hakari` to work properly.
//!
//! ### 1. Initialize the workspace-hack
//!
//! Initialize a workspace-hack crate for a workspace at path `my-workspace-hack`:
//!
//! ```sh
//! cargo hakari init my-workspace-hack
//! ```
//!
//! <p align="center">
//! <img src="https://user-images.githubusercontent.com/180618/135726175-dc00dd0c-68a1-455f-a13d-0dd24f545ca6.png">
//! </p>
//!
//! ### 2. Generate the `Cargo.toml`
//!
//! Generate or update the contents of a workspace-hack crate:
//!
//! ```sh
//! cargo hakari generate
//! ```
//!
//! ### 3. Add dependencies to the workspace-hack
//!
//! Add the workspace-hack crate as a dependency to all other workspace crates:
//!
//! ```sh
//! cargo hakari manage-deps
//! ```
//!
//! <p align="center">
//! <img src="https://user-images.githubusercontent.com/180618/135725773-c71fc4cd-8b7d-4a8e-b97c-d84a2b3b3662.png">
//! </p>
//!
//! ## Making hakari work well
//!
//! These are things that are not absolutely necessary to do, but will make `cargo hakari` work
//! better.
//!
//! ### 1. Update the hakari config
//!
//! Open up `.config/hakari.toml`, then:
//!
//! * uncomment or add commonly-used developer platforms
//! * read the note about the resolver, and strongly consider
//!   [setting `resolver = "2"`](https://blog.rust-lang.org/2021/03/25/Rust-1.51.0.html#cargos-new-feature-resolver)
//!   in your workspace's `Cargo.toml`.
//!
//! Remember to run `cargo hakari generate` after changing the config.
//!
//! ### 2. Keep the workspace-hack up-to-date in CI
//!
//! Run the following commands in CI:
//!
//! ```sh
//! cargo hakari generate --diff  # workspace-hack Cargo.toml is up-to-date
//! cargo hakari manage-deps --dry-run  # all workspace crates depend on workspace-hack
//! ```
//!
//! If either of these commands exits with a non-zero status, you can choose to fail CI or produce
//! a warning message.
//!
//! For an example, see [this GitHub action used by
//! `cargo-guppy`](https://github.com/facebookincubator/cargo-guppy/blob/main/.github/workflows/hakari.yml).
//!
//! All `cargo hakari` commands take a `--quiet` option to suppress output, though showing diff
//! output in CI is often useful.
//!
//! ## Information about the workspace-hack
//!
//! The commands in this section provide information about components in the workspace-hack.
//!
//! ### Why is a dependency in the workspace-hack?
//!
//! Print out information about why a dependency is present in the workspace-hack:
//!
//! ```sh
//! cargo hakari explain <dependency-name>
//! ```
//!
//! <p align="center">
//! <img src="https://user-images.githubusercontent.com/180618/144933657-c45cf719-ecaf-49e0-b2c7-c8d12adf11c0.png" width=550>
//! </p>
//!
//! ### Does the workspace-hack ensure that each dependency is built with exactly one feature set?
//!
//! ```sh
//! cargo hakari verify
//! ```
//!
//! If some dependencies are built with more than one feature set, this command will print out
//! details about them. **This is always a bug**---if you encounter it, [a bug report] with more
//! information would be greatly appreciated!
//!
//! [a bug report]: https://github.com/facebookincubator/cargo-guppy/issues/new
//!
//! ###
//! ## Publishing a crate
//!
//! If you publish crates to `crates.io` or other registries, see the
//! [`publishing` module](https://docs.rs/cargo-hakari/latest/cargo_hakari/publishing).
//!
//! ## Disabling and uninstalling
//!
//! Disable the workspace-hack crate temporarily by removing generated lines from `Cargo.toml`.
//! (Re-enable by running `cargo hakari generate`.)
//!
//! ```sh
//! cargo hakari disable
//! ```
//!
//! Remove the workspace-hack crate as a dependency from all other workspace crates:
//!
//! ```sh
//! cargo hakari remove-deps
//! ```
//!
//! <p align="center">
//! <img src="https://user-images.githubusercontent.com/180618/135726181-9fe86782-6471-4a1d-a511-a6c55dffbbd7.png">
//! </p>
//!
//! # Configuration
//!
//! `cargo hakari` is configured through `.config/hakari.toml` at the root of the workspace. Running
//! `cargo hakari init` causes a new file to be created at this location.
//!
//! Example configuration:
//!
//! ```toml
//! ## The name of the package used for workspace-hack unification.
//! hakari-package = "workspace-hack"
//! ## Cargo resolver version in use -- version 2 is highly recommended.
//! resolver = "2"
//!
//! ## Format for `workspace-hack = ...` lines in other Cargo.tomls. Version 2 requires cargo-hakari
//! ## 0.9.8 or above.
//! dep-format-version = "2"
//!
//! ## Add triples corresponding to platforms commonly used by developers here.
//! ## https://doc.rust-lang.org/rustc/platform-support.html
//! platforms = [
//!     ## "x86_64-unknown-linux-gnu",
//!     ## "x86_64-apple-darwin",
//!     ## "x86_64-pc-windows-msvc",
//! ]
//!
//! ## Write out exact versions rather than specifications. Set this to true if version numbers in
//! ## `Cargo.toml` and `Cargo.lock` files are kept in sync, e.g. in some configurations of
//! ## https://dependabot.com/.
//! ## exact-versions = false
//! ```
//!
//! For more options, including how to exclude crates from the output, see the
//! [`config` module](https://docs.rs/cargo-hakari/latest/cargo_hakari/config).
//!
//! # Stability guarantees
//!
//! `cargo-hakari` follows semantic versioning, where the public API is the command-line interface.
//!
//! Within a given series, the command-line interface will be treated as append-only.
//! The generated `Cargo.toml` will also be kept the same unless:
//! * a new config option is added, in which case the different output will be gated on the new
//!   option, or
//! * there is a bugfix involved.

mod cargo_cli;
mod command;
mod docs;
mod helpers;
mod output;
mod publish;

pub use docs::*;

// Not part of the stable API.
#[doc(hidden)]
pub use command::Args;
