# cargo-hakari

[![cargo-hakari on crates.io](https://img.shields.io/crates/v/cargo-hakari)](https://crates.io/crates/cargo-hakari) [![Documentation (latest release)](https://docs.rs/cargo-hakari/badge.svg)](https://docs.rs/cargo-hakari/) [![Documentation (main)](https://img.shields.io/badge/docs-main-brightgreen)](https://facebookincubator.github.io/cargo-guppy/rustdoc/cargo-hakari/) [![License](https://img.shields.io/badge/license-Apache-green.svg)](../../LICENSE-APACHE) [![License](https://img.shields.io/badge/license-MIT-green.svg)](../../LICENSE-MIT)

Manage workspace-hack crates.

## Configuration

`cargo-hakari` is configured through `Hakari.toml` at the root of the workspace.

Example configuration:

```toml
# Which version of the Cargo resolver to use. This should match the "resolver" key in
# `Cargo.toml`.
resolver = "2"
# The name of the package used for workspace-hack unification.
hakari-package = "workspace-hack"

# The platforms to perform resolution on. Can be left blank to perform a unified resolution
# across all platforms. (This may lead to potentially unexpected results.)
platforms = [
    { triple = "x86_64-unknown-linux-gnu", target-features = "unknown" }
]

# Options to control Hakari output.
[output]
# Write out exact versions rather than specifications. Set this to true if version numbers in
# `Cargo.toml` and `Cargo.lock` files are kept in sync, e.g. in some configurations of
# https://dependabot.com/.
# exact-versions = false

# Write out a summary of builder options as a comment in the workspace-hack Cargo.toml.
# builder-summary = false
```

For more options, see the (TODO XXX config file).

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
