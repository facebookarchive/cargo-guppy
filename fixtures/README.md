# Fixtures for cargo-guppy

This directory contains interesting test corpuses used within the cargo-guppy codebase.

The fixtures are organized into several folders.

## `cargo metadata` output

* `determinator-paths`: determinator path matching across platforms.
* `small`: relatively simple examples that cover basic and some edge case functionality
* `large`: complex examples pulled from real-world Rust repositories, that test a variety of edge cases
* `invalid`: examples that are [*representable*](https://oleb.net/blog/2018/03/making-illegal-states-unrepresentable/)
  as cargo metadata (i.e. they are valid JSON and follow the general schema) but are *invalid* in some way; `cargo
  metadata` should never be able to generate these
* `workspace`: real workspaces, used for comparison testing with Cargo
