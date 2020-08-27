# guppy

[![guppy on crates.io](https://img.shields.io/crates/v/guppy)](https://crates.io/crates/guppy) [![Documentation (latest release)](https://docs.rs/guppy/badge.svg)](https://docs.rs/guppy/) [![Documentation (master)](https://img.shields.io/badge/docs-master-brightgreen)](https://facebookincubator.github.io/cargo-guppy/rustdoc/guppy/) [![License](https://img.shields.io/badge/license-Apache-green.svg)](../LICENSE-APACHE) [![License](https://img.shields.io/badge/license-MIT-green.svg)](../LICENSE-MIT)

Track and query Cargo dependency graphs.

`guppy` provides a Rust interface to run queries over Cargo dependency graphs. `guppy` parses
the output of  [`cargo metadata`](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html),
then presents a graph interface over it.

## Usage

Add the following to `Cargo.toml`:

```toml
[dependencies]
guppy = "0.5"
```

## Optional features

* `proptest010`: Support for [property-based testing](https://jessitron.com/2013/04/25/property-based-testing-what-is-it/)
  using the [`proptest`](https://altsysrq.github.io/proptest-book/intro.html) framework.
* `rayon1`: Support for parallel iterators through [Rayon](docs.rs/rayon/1) (preliminary work
  so far, more parallel iterators to be added in the future).
* `summaries`: Support for writing out [build summaries](https://github.com/facebookincubator/cargo-guppy/tree/master/guppy-summaries).

## Examples

Print out all direct dependencies of a package:

```rust
use guppy::{CargoMetadata, PackageId};

// `guppy` accepts `cargo metadata` JSON output. Use a pre-existing fixture for these examples.
let metadata = CargoMetadata::parse_json(include_str!("../../fixtures/small/metadata1.json")).unwrap();
let package_graph = metadata.build_graph().unwrap();

// `guppy` provides several ways to get hold of package IDs. Use a pre-defined one for this
// example.
let package_id = PackageId::new("testcrate 0.1.0 (path+file:///fakepath/testcrate)");

// The `metadata` method returns information about the package, or `None` if the package ID
// wasn't recognized.
let package = package_graph.metadata(&package_id).unwrap();

// `direct_links` returns all direct dependencies of a package.
for link in package.direct_links() {
    // A dependency link contains `from()`, `to()` and information about the specifics of the
    // dependency.
    println!("direct dependency: {}", link.to().id());
}
```

For more examples, see
[the `examples` directory](https://github.com/facebookincubator/cargo-guppy/tree/master/guppy/examples).

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
