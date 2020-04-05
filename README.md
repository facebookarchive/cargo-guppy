# cargo-guppy: track and query dependency graphs

[![Build Status](https://circleci.com/gh/calibra/cargo-guppy/tree/master.svg?style=shield)](https://circleci.com/gh/calibra/cargo-guppy/tree/master) [![License](https://img.shields.io/badge/license-Apache-green.svg)](LICENSE-APACHE) [![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE-MIT)

This repository contains the source code for:
* [`guppy`](guppy): a library for performing queries on Cargo dependency graphs [![guppy on crates.io](https://img.shields.io/crates/v/guppy)](https://crates.io/crates/guppy) [![Documentation (latest release)](https://docs.rs/guppy/badge.svg)](https://docs.rs/guppy/) [![Documentation (master)](https://img.shields.io/badge/docs-master-59f)](https://calibra.github.io/cargo-guppy/guppy/)
* [`target-spec`](target-spec): an evaluator for `Cargo.toml` target specifications [![target-spec on crates.io](https://img.shields.io/crates/v/target-spec)](https://crates.io/crates/target-spec) [![Documentation (latest release)](https://docs.rs/target-spec/badge.svg)](https://docs.rs/target-spec/) [![Documentation (master)](https://img.shields.io/badge/docs-master-59f)](https://calibra.github.io/cargo-guppy/target_spec/)
* [`cargo-guppy`](cargo-guppy): a command-line frontend for the `guppy` library

The code in this repository is in a **pre-release** state and is under active development.

## Use cases

`guppy` and `cargo-guppy` can be used to solve many practical problems related to dependency graphs in large Rust
codebases. Some examples -- all of these are available through the `guppy` library, and will eventually be supported in
the `cargo-guppy` CLI as well:

* track existing dependencies for a crate or workspace
* query direct or transitive dependencies of a subset of packages — useful when some packages have greater assurance or
  reliability requirements
* figure out what's causing a particular crate to be included as a dependency
* iterate over reverse dependencies of a crate in topological order
* iterate over some or all links (edges) in a dependency graph, querying if the link is a build, dev or regular
  dependency
* evaluation of target specs for [platform-specific dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#platform-specific-dependencies)
* filter out dev-only dependencies while performing queries, since they're typically not included in release builds
* print out a `dot` graph for a subset of crates, for formatting with [graphviz](https://www.graphviz.org/)

Still to come:

* receive CI feedback if a dependency is added, updated or removed
* receive CI feedback if a package goes from not being included in a high-assurance subset to being included
* queries based on features
* a command-line query language

This code has been written for the [Libra Core](https://github.com/libra/libra) project, but it may be useful for other
large Rust projects.

## Design

`guppy` is written on top of the excellent [petgraph](https://github.com/petgraph/petgraph) library. It is a separate
codebase from `cargo`, depending only on the stable [`cargo
metadata`](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html) format. (Some other tools in this space like
[`cargo-tree`](https://github.com/sfackler/cargo-tree) use cargo internals directly.)

## Contributing

See the [CONTRIBUTING](CONTRIBUTING.md) file for how to help out.

## License

This project is available under the terms of either the [Apache 2.0 license](LICENSE-APACHE) or the [MIT
license](LICENSE-MIT).
