# Changelog

## Unreleased

### Changed

- MSRV updated to Rust 1.56.

## [0.8.0] - 2022-02-13

### Changed

- Public dependency version bump: `guppy` updated to 0.13.0.

## [0.7.0] - 2021-11-23

### Added

- `determinator` now works with both forward and backslash-based changed paths on Windows.

### Changed

- Public dependency version bump: `guppy` updated to 0.12.0.

## [0.6.0] - 2021-10-01

### Changed

- Public dependency version bump: `guppy` updated to 0.11.1.
- MSRV updated to Rust 1.53.

## [0.5.1] - 2021-09-13

### Changed

- Public dependency version bump: `guppy` updated to 0.10.1.
- MSRV updated to Rust 1.51.

## [0.5.0] - 2021-09-13

(This release was never published because it was based on `guppy 0.10.0`, which was yanked.)

## [0.4.0] - 2021-03-11

### Changed

- Public dependency version bump: `guppy` updated to 0.9.0.

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

[0.8.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/determinator-0.8.0
[0.7.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/determinator-0.7.0
[0.6.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/determinator-0.6.0
[0.5.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/determinator-0.5.1
[0.5.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/determinator-0.5.0
[0.4.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/determinator-0.4.0
[0.3.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/determinator-0.3.0
[0.2.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/determinator-0.2.1
[0.2.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/determinator-0.2.0
[0.1.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/determinator-0.1.1
[0.1.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/determinator-0.1.0
