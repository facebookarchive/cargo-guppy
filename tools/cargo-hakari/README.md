# cargo-hakari

[![cargo-hakari on crates.io](https://img.shields.io/crates/v/cargo-hakari)](https://crates.io/crates/cargo-hakari) [![Documentation (latest release)](https://docs.rs/cargo-hakari/badge.svg)](https://docs.rs/cargo-hakari/) [![Documentation (main)](https://img.shields.io/badge/docs-main-brightgreen)](https://facebookincubator.github.io/cargo-guppy/rustdoc/cargo_hakari/) [![License](https://img.shields.io/badge/license-Apache-green.svg)](../../LICENSE-APACHE) [![License](https://img.shields.io/badge/license-MIT-green.svg)](../../LICENSE-MIT)

`cargo hakari` is a command-line application to manage workspace-hack crates.

For an explanation of what workspace-hack packages are and how they can help, see the
[`about` module](https://docs.rs/cargo-hakari/*/cargo_hakari/about).

## Platform support

* **Unix platforms**: Hakari works and is supported.
* **Windows**: Hakari works and outputs file paths with forward slashes for
  consistency with Unix. CRLF line endings are not supported in the workspace-hack's
  `Cargo.toml` -- it is recommended that repositories disable automatic line ending conversion.
  [Here's how to do it in Git](https://stackoverflow.com/a/10017566).
  (Pull requests to improve this are welcome.)

## Installation and usage

All of the below commands take options that control their behavior.

To install, run:

```sh
cargo install cargo-hakari
```

To update, run:

```sh
cargo install --force cargo-hakari
```

If `$HOME/.cargo/bin` is in your `PATH`, the `cargo hakari` command will be available.

### Usage

Initialize a workspace-hack crate for a workspace at path `my-workspace-hack`:

```sh
cargo hakari init my-workspace-hack
```

<p align="center">
<img src="https://user-images.githubusercontent.com/180618/135725771-9395db82-73fa-4c0c-8908-8bfe5b1b0465.png">
</p>

Generate or update the contents of a workspace-hack crate.

```sh
cargo hakari generate
```

Add the workspace-hack crate as a dependency to all other workspace crates:

```sh
cargo hakari manage-deps
```

<p align="center">
<img src="https://user-images.githubusercontent.com/180618/135725773-c71fc4cd-8b7d-4a8e-b97c-d84a2b3b3662.png">
</p>

Publish a crate that currently depends on the workspace-hack crate (`cargo publish` can't be
used in this circumstance):

```sh
cargo hakari publish -p <crate>
```

### Disabling and uninstalling

Disable the workspace-hack crate temporarily by removing generated contents. (Re-enable by
running `cargo hakari generate`).

```sh
cargo hakari disable
```

Remove the workspace-hack crate as a dependency from all other workspace crates:

```sh
cargo hakari remove-deps
```

<p align="center">
<img src="https://user-images.githubusercontent.com/180618/135725775-5c874139-8188-40b3-8fd1-6bf65f19985f.png">
</p>

## Configuration

`cargo hakari` is configured through `.guppy/hakari.toml` at the root of the workspace.

Example configuration:

```toml
## The name of the package used for workspace-hack unification.
hakari-package = "workspace-hack"
## Cargo resolver version in use -- version 2 is highly recommended.
resolver = "2"

## Add triples corresponding to platforms commonly used by developers here.
## https://doc.rust-lang.org/rustc/platform-support.html
platforms = [
    ## "x86_64-unknown-linux-gnu",
    ## "x86_64-apple-darwin",
    ## "x86_64-pc-windows-msvc",
]

## Options to control Hakari output.
[output]
## Write out exact versions rather than specifications. Set this to true if version numbers in
## `Cargo.toml` and `Cargo.lock` files are kept in sync, e.g. in some configurations of
## https://dependabot.com/.
## exact-versions = false
```

For more options, see the [`config` module](https://docs.rs/cargo-hakari/*/cargo_hakari/config).

## Contributing

See the [CONTRIBUTING](../../CONTRIBUTING.md) file for how to help out.

## License

This project is available under the terms of either the [Apache 2.0 license](../../LICENSE-APACHE) or the [MIT
license](../../LICENSE-MIT).

<!--
README.md is generated from README.tpl by cargo readme. To regenerate:

cargo install cargo-readme
cargo readme > README.md
-->
