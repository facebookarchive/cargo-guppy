# cargo-hakari

[![cargo-hakari on crates.io](https://img.shields.io/crates/v/cargo-hakari)](https://crates.io/crates/cargo-hakari) [![Documentation (latest release)](https://docs.rs/cargo-hakari/badge.svg)](https://docs.rs/cargo-hakari/) [![Documentation (main)](https://img.shields.io/badge/docs-main-brightgreen)](https://facebookincubator.github.io/cargo-guppy/rustdoc/cargo_hakari/) [![License](https://img.shields.io/badge/license-Apache-green.svg)](../../LICENSE-APACHE) [![License](https://img.shields.io/badge/license-MIT-green.svg)](../../LICENSE-MIT)

`cargo hakari` is a command-line application to manage workspace-hack crates. Use it to speed up
local `cargo build` and `cargo check` commands by **15-95%**, and cumulatively by
**20-25% or more**.

For an explanation of what workspace-hack packages are and how they make your builds faster, see
the [`about` module](https://docs.rs/cargo-hakari/*/cargo_hakari/about).

## Examples

The `cargo-guppy` repository uses a workspace-hack crate managed by `cargo hakari`. [See the
generated `Cargo.toml`.](https://github.com/facebookincubator/cargo-guppy/blob/main/workspace-hack/Cargo.toml)

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
<img src="https://user-images.githubusercontent.com/180618/135726175-dc00dd0c-68a1-455f-a13d-0dd24f545ca6.png">
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

### Keeping the workspace-hack crate up-to-date

Run the following commands in CI:

```sh
cargo hakari generate --diff  # workspace-hack Cargo.toml is up-to-date
cargo hakari manage-deps --dry-run  # all workspace crates depend on workspace-hack
```

If either of these commands exits with a non-zero status, you can choose to fail CI or produce
a warning message.

For an example, see [this GitHub action used by
`cargo-guppy`](https://github.com/facebookincubator/cargo-guppy/blob/main/.github/workflows/hakari.yml).

All `cargo hakari` commands take a `--quiet` option to suppress output, though showing diff
output in CI is often useful.

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
<img src="https://user-images.githubusercontent.com/180618/135726181-9fe86782-6471-4a1d-a511-a6c55dffbbd7.png">
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

## Write out exact versions rather than specifications. Set this to true if version numbers in
## `Cargo.toml` and `Cargo.lock` files are kept in sync, e.g. in some configurations of
## https://dependabot.com/.
## exact-versions = false
```

For more options, including how to exclude crates from the output, see the
[`config` module](https://docs.rs/cargo-hakari/*/cargo_hakari/config).

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
