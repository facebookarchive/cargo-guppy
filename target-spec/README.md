# target-spec

[![target-spec on crates.io](https://img.shields.io/crates/v/target-spec)](https://crates.io/crates/target-spec) [![Documentation (latest release)](https://docs.rs/target-spec/badge.svg)](https://docs.rs/target-spec/) [![Documentation (master)](https://img.shields.io/badge/docs-master-brightgreen)](https://facebookincubator.github.io/cargo-guppy/target_spec/) [![License](https://img.shields.io/badge/license-Apache-green.svg)](../LICENSE-APACHE) [![License](https://img.shields.io/badge/license-MIT-green.svg)](../LICENSE-MIT)

Evaluate `Cargo.toml` target specifications against platform triples.

Cargo supports
[platform-specific dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#platform-specific-dependencies).
These dependencies can be specified in one of two ways:

```toml
# 1. As Rust-like `#[cfg]` syntax.
[target.'cfg(all(unix, target_arch = "x86_64"))'.dependencies]
native = { path = "native/x86_64" }

# 2. Listing out the full target triple.
[target.x86_64-pc-windows-gnu.dependencies]
winhttp = "0.4.0"
```

`target-spec` provides the `eval` API which can be used to figure out whether such a
dependency will be included on a particular platform.

```rust
use target_spec::eval;

// Evaluate Rust-like `#[cfg]` syntax.
let cfg_target = "cfg(all(unix, target_arch = \"x86_64\"))";
assert_eq!(eval(cfg_target, "x86_64-unknown-linux-gnu"), Ok(Some(true)));
assert_eq!(eval(cfg_target, "i686-unknown-linux-gnu"), Ok(Some(false)));
assert_eq!(eval(cfg_target, "x86_64-pc-windows-msvc"), Ok(Some(false)));

// Evaluate a full target-triple.
assert_eq!(eval("x86_64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"), Ok(Some(true)));
assert_eq!(eval("x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc"), Ok(Some(false)));
```

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
