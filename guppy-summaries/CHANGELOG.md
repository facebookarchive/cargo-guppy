# Changelog

## Unreleased

### Changed

- MSRV updated to Rust 1.56.

## [0.6.1] - 2021-11-23

### Changed

- The `toml` crate is now built with the `preserve_order` feature.
  - This feature ensures that the key ordering in metadata is preserved.

## [0.6.0] - 2021-11-23

This is a minor breaking change that should not affect most consumers.

### Changed

- `SummaryWithMetadata` is now simply `Summary`, and no longer takes a type parameter.
  - `metadata` is now a `toml::value::Table`.
- `path_forward_slashes` is no longer exposed as a helper.

## [0.5.1] - 2021-10-01

### Added

- `SummaryId` now implements `Display`, printing out the ID as a TOML inline table.
- A new convenience module `path_forward_slashes` is provided to serialize and deserialize paths using
  forward slashes.

## [0.5.0] - 2021-09-13

### Changed

- Public dependency version bumps:
  - `semver` updated to 1.0.
  - `diffus` updated to 0.10.0.

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

[0.6.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-summaries-0.6.1
[0.6.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-summaries-0.6.0
[0.5.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-summaries-0.5.1
[0.5.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-summaries-0.5.0
[0.4.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-summaries-0.4.0
[0.3.2]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-summaries-0.3.2
[0.3.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-summaries-0.3.1
[0.3.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-summaries-0.3.0
[0.2.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-summaries-0.2.0
[0.1.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-summaries-0.1.0
