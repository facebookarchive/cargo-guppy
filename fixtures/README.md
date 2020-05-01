# Fixtures for cargo-guppy

This directory contains interesting test corpuses used within the cargo-guppy codebase.

The fixtures are organized into several folders.

## `cargo metadata` output

* `small`: relatively simple examples that cover basic and some edge case functionality
* `large`: complex examples pulled from real-world Rust repositories, that test a variety of edge cases
* `invalid`: examples that are *representable* as cargo metadata (i.e. they are valid JSON and follow the general
  schema) but are *invalid* in some way; `cargo metadata` should never be able to generate these
