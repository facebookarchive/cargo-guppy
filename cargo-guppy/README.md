# cargo-guppy

[![Documentation (master)](https://img.shields.io/badge/docs-master-brightgreen)](https://facebookincubator.github.io/cargo-guppy/cargo_guppy/)
[![License](https://img.shields.io/badge/license-Apache-green.svg)](../LICENSE-APACHE)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](../LICENSE-MIT)

A command-line frontend for `guppy`.

`cargo-guppy` provides a frontend for running `guppy` queries.

### Installing

`cargo-guppy` is currently a work in progress, and not yet on `crates.io`. To install it, ensure
you have `cargo` installed (preferably through [rustup](https://rustup.rs/)), then run:

```bash
cargo install --git https://github.com/facebookincubator/cargo-guppy cargo-guppy
```

This will make the `cargo guppy` command available.

### Commands

The list of commands is not currently stable and is subject to change.

#### Query commands

* `select`: query packages and their transitive dependencies
* `resolve-cargo`: query packages and features as would be built by cargo
* `subtree-size`: print dependencies along with their unique subtree size
* `dups`: print duplicate packages

#### Diff commands

* `diff`: perform a diff of two `cargo metadata` JSON outputs
* `diff-summaries`: perform a diff of two [summaries](https://github.com/facebookincubator/cargo-guppy/tree/master/guppy-summaries)

#### Workspace manipulations

* `mv`: move crates to a new location in a workspace, updating paths along the way

## Contributing

See the [CONTRIBUTING](../CONTRIBUTING.md) file for how to help out.

## License

This project is available under the terms of either the [Apache 2.0 license](../LICENSE-APACHE) or the [MIT
license](../LICENSE-MIT).

<!--
README.md is generated from README.tpl by cargo readme. To regenerate:

cargo install cargo-readme
cargo readme > README.md
-->
