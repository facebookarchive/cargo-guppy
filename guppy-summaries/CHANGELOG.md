# Changelog

## Unreleased

### Changed

- MSRV updated to Rust 1.51.

## [0.4.0] - 2021-02-23

### Changed

- `guppy-summaries` now uses [`camino`](https://crates.io/crates/camino) `Utf8Path` and `Utf8PathBuf` wrappers. These
  wrappers provide type-level assertions that returned paths are valid UTF-8.

## [0.3.2] - 2021-02-04

### Fixed

- `SummarySource` paths are now always output with forward slashes, including on Windows.

## [0.3.1] - 2020-12-09

### Added

- Support for serializing `SummaryDiff` instances (thanks @mimoo).

## [0.3.0] - 2020-12-02

### Changed

- Updated semver to 0.11.

## [0.2.0] - 2020-06-20

### Changed

- Move diff-related types into a new `diff` module.
- Don't export `Summary` as a default type alias any more.
- Remove `parse_with_metadata` in favor of making `parse` generic.

## [0.1.0] - 2020-06-12

Initial release.

[0.4.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-summaries-0.4.0
[0.3.2]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-summaries-0.3.2
[0.3.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-summaries-0.3.1
[0.3.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-summaries-0.3.0
[0.2.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-summaries-0.2.0
[0.1.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-summaries-0.1.0
