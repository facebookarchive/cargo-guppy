# Changelog

## Unreleased

### Changed

- MSRV updated to Rust 1.51.

## [0.4.0] - 2021-03-11

### Changed

- `guppy` updated to 0.9.0.

## [0.3.0] - 2021-02-23

### Changed

- `determinator` now uses [`camino`](https://crates.io/crates/camino) `Utf8Path` and `Utf8PathBuf` wrappers. These wrappers
  provide type-level assertions that returned paths are valid UTF-8.

## [0.2.1] - 2021-02-04

### Added

* Experimental Windows support. There may still be bugs around path normalization: please [report them](https://github.com/facebookincubator/cargo-guppy/issues/new).

## [0.2.0] - 2021-02-03

### Changed

* `guppy` updated to 0.7.

## [0.1.1] - 2020-12-02

Initial release.

### Fixed

* Fixed `Cargo.toml` package metadata.

## [0.1.0] - 2020-12-02

(This version was not released to crates.io.)

### Added

* Support for determining which packages in a workspace have changed between two commits.
* Path-based and package-based custom rules, including a default set of rules for files like `rust-toolchain` and `Cargo.lock`.
* A `Paths0` wrapper to make it easier to retrieve changes from source control.

[0.4.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/determinator-0.4.0
[0.3.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/determinator-0.3.0
[0.2.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/determinator-0.2.1
[0.2.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/determinator-0.2.0
[0.1.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/determinator-0.1.1
[0.1.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/determinator-0.1.0
