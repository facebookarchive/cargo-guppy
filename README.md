# cargo-guppy: track and query dependency graphs

[![Build Status](https://github.com/facebookincubator/cargo-guppy/workflows/CI/badge.svg?branch=master)]((https://github.com/facebookincubator/cargo-guppy/actions?query=workflow%3ACI+branch%3Amaster))
[![License](https://img.shields.io/badge/license-Apache-green.svg)](LICENSE-APACHE) [![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE-MIT)

This repository contains the source code for:
* [`guppy`](guppy): a library for performing queries on Cargo dependency graphs [![guppy on crates.io](https://img.shields.io/crates/v/guppy)](https://crates.io/crates/guppy) [![Documentation (latest release)](https://docs.rs/guppy/badge.svg)](https://docs.rs/guppy/) [![Documentation (master)](https://img.shields.io/badge/docs-master-59f)](https://facebookincubator.github.io/cargo-guppy/rustdoc/guppy/)
* libraries used by guppy:
  * [`guppy-summaries`](guppy-summaries): a library for managing build summaries listing packages and features [![guppy-summaries on crates.io](https://img.shields.io/crates/v/guppy-summaries)](https://crates.io/crates/guppy-summaries) [![Documentation (latest release)](https://docs.rs/guppy-summaries/badge.svg)](https://docs.rs/guppy/) [![Documentation (master)](https://img.shields.io/badge/docs-master-59f)](https://facebookincubator.github.io/cargo-guppy/rustdoc/guppy_summaries/)
  * [`target-spec`](target-spec): an evaluator for `Cargo.toml` target specifications [![target-spec on crates.io](https://img.shields.io/crates/v/target-spec)](https://crates.io/crates/target-spec) [![Documentation (latest release)](https://docs.rs/target-spec/badge.svg)](https://docs.rs/target-spec/) [![Documentation (master)](https://img.shields.io/badge/docs-master-59f)](https://facebookincubator.github.io/cargo-guppy/rustdoc/target_spec/)
* tools built on top of guppy:
  * [`determinator`](tools/determinator): figure out what packages changed between two revisions [![determinator on crates.io](https://img.shields.io/crates/v/determinator)](https://crates.io/crates/determinator) [![Documentation (latest release)](https://docs.rs/determinator/badge.svg)](https://docs.rs/determinator/) [![Documentation (master)](https://img.shields.io/badge/docs-master-59f)](https://facebookincubator.github.io/cargo-guppy/rustdoc/determinator/)
  * [`cargo-guppy`](cargo-guppy): a command-line frontend for the `guppy` library [![Documentation (master)](https://img.shields.io/badge/docs-master-59f)](https://facebookincubator.github.io/cargo-guppy/rustdoc/cargo_guppy/)
* and a number of [internal tools](internal-tools) and [test fixtures](fixtures) used to verify that `guppy` behaves correctly.

## Use cases

`guppy` and `cargo-guppy` can be used to solve many practical problems related to dependency graphs in large Rust
codebases. Some examples -- all of these are available through the `guppy` library, and will eventually be supported in
the `cargo-guppy` CLI as well:

* track existing dependencies for a crate or workspace
* query direct or transitive dependencies of a subset of packages — useful when some packages have greater assurance or
  reliability requirements
* figure out what's causing a particular crate to be included as a dependency
* iterate over reverse dependencies of a crate in [topological order](https://en.wikipedia.org/wiki/Topological_sorting)
* iterate over some or all links (edges) in a dependency graph, querying if the link is a build, dev or regular
  dependency
* filter out dev-only dependencies while performing queries
* perform queries based on [Cargo features](https://doc.rust-lang.org/cargo/reference/features.html)
* simulate Cargo builds and return what packages and features would be built by it
* evaluate target specs for [platform-specific dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#platform-specific-dependencies)
* generate *summary files* for Cargo builds, which can be used to:
  * receive CI feedback if a dependency is added, updated or removed, or if new features are added
  * receive CI feedback if a package is added to a high-assurance subset, or if any new features are enabled in
    an existing package in that subset. This can be used to flag those changes for extra scrutiny.
* print out a `dot` graph for a subset of crates, for formatting with [graphviz](https://www.graphviz.org/)

Still to come:

* a command-line query language

## Development status

The core `guppy` code in this repository is considered **mostly complete** and the API is mostly stable.

We have ideas for a number of tools on top of guppy, and those are still are under **active development**. Tool requirements may cause further changes in the API, but the goal will be to avoid extensive overhauls.

`guppy`'s simulation of Cargo builds is almost perfect. The known differences that remain are minor, and are mostly internal inconsistencies in Cargo. Comparison testing with `guppy` has already found several bugs in Cargo, for example:
* [v2 resolver: a proc macro being specified with the key "proc_macro" vs "proc-macro" causes different results](https://github.com/rust-lang/cargo/issues/8315)
* [v2 resolver: different handling for inactive, optional dependencies based on how they're specified](https://github.com/rust-lang/cargo/issues/8316)

## Production users

`cargo-guppy` is extensively used by the [Libra Core](https://github.com/libra/libra) project.

`guppy` is used for [several lint checks](https://github.com/libra/libra/blob/master/devtools/x/src/lint/guppy.rs). This includes basic rules that look at every workspace package separately:
* every package has fields like `license` specified
* crate names and paths should use `-` instead of `_`

to more complex rules about the overall dependency graph, such as:
* some third-party dependencies are banned from the workspace entirely, or only from default builds
* every workspace package depends on a `workspace-hack` crate (similar to [rustc-workspace-hack](https://github.com/rust-lang/rust/tree/master/src/tools/rustc-workspace-hack))
* for any given third-party dependency, the workspace only depends on one version of it directly (transitive dependencies to other versions are still allowed)
* every workspace package is categorized as either *production* or *test-only*, and the linter checks that test-only crates are not included in production builds
* support for *overlay features*, which allow test-only code to be:
  * included in crates (similar to [the `#[cfg(test)]` annotation](https://doc.rust-lang.org/book/ch11-03-test-organization.html#the-tests-module-and-cfgtest))
  * depended on by test-only code in other crates (`#[cfg(test)]` does not allow this)
  * but guaranteed to be excluded from production builds

In addition, `guppy-summaries` is used to generate build summaries of packages and features (particularly for [high-security subsets](https://en.wikipedia.org/wiki/Trusted_computing_base) of the codebase), and changes to these sets are flagged by Libra's CI ([example](https://github.com/libra/libra/pull/5799#issuecomment-682221102)).

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
