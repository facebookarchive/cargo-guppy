# Changelog

## [0.1.1] - 2021-02-04

### Added

* Experimental Windows support. There may still be bugs around path normalization: please [report them](https://github.com/facebookincubator/cargo-guppy/issues/new).

### Fixed

* Fixed Cargo.toml output for path dependencies.
* Return an error for non-Unicode paths instead of silently producing incorrect paths.

## [0.1.0] - 2021-02-03

Initial release.

[0.1.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/hakari-0.1.1
[0.1.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/hakari-0.1.0
