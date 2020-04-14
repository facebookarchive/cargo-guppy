# guppy

[![guppy on crates.io](https://img.shields.io/crates/v/guppy)](https://crates.io/crates/guppy) [![Documentation (latest release)](https://docs.rs/guppy/badge.svg)](https://docs.rs/guppy/) [![Documentation (master)](https://img.shields.io/badge/docs-master-brightgreen)](https://facebookincubator.github.io/cargo-guppy/guppy/) [![License](https://img.shields.io/badge/license-Apache-green.svg)](../LICENSE-APACHE) [![License](https://img.shields.io/badge/license-MIT-green.svg)](../LICENSE-MIT)

Track and query Cargo dependency graphs.

`guppy` provides a Rust interface to run queries over Cargo dependency graphs. `guppy` parses
the output of  [`cargo metadata`](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html),
then presents a graph interface over it.

## Usage

Add the following to `Cargo.toml`:

```toml
[dependencies]
guppy = "0.3"
```

## Examples

Print out all direct dependencies of a package:

```rust
use guppy::graph::PackageGraph;
use guppy::PackageId;

// `guppy` accepts `cargo metadata` JSON output. Use a pre-existing fixture for these examples.
let fixture = include_str!("../fixtures/small/metadata1.json");
let package_graph = PackageGraph::from_json(fixture).unwrap();

// `guppy` provides several ways to get hold of package IDs. Use a pre-defined one for this
// example.
let package_id = PackageId::new("testcrate 0.1.0 (path+file:///fakepath/testcrate)");
// dep_links returns all direct dependencies of a package, and it returns `None` if the package
// ID isn't recognized.
for link in package_graph.dep_links(&package_id).unwrap() {
    // A dependency link contains `from`, `to` and `edge`. The edge has information about e.g.
    // whether this is a build dependency.
    println!("direct dependency: {}", link.to.id());
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
