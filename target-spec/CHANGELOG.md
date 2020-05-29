# Changelog

## [0.2.4] - 2020-05-06

### Added
- New feature `proptest010` to generate random platforms for property testing.

## [0.2.3] - 2020-04-15

### Fixed
- Better handling of unknown flags.
  - Unknown flags now evaluate to false instead of erroring out.
  - Added `Platform::add_flags` to allow setting flags that evaluate to true.

These changes were prompted by how [`cargo-web`](https://github.com/koute/cargo-web) sets the `cargo_web` flag to
true for `cargo web build`.

## 0.2.2

This was mistakenly published and was yanked.

## [0.2.1] - 2020-04-07
### Changed
- Updated repository links.

## [0.2.0] - 2020-04-05
### Added
- Added support for parsing specs and platforms separately from evaluating them, making error-less evaluation possible.
- Added support for target features, including situations when target features are unknown.

### Changed
- Switched to [`cfg-expr`](https://github.com/EmbarkStudios/cfg-expr) as the backend for `cfg()` expressions.

## [0.1.0] - 2020-03-20
- Initial release.

[0.2.4]: https://github.com/facebookincubator/cargo-guppy/releases/tag/target-spec-0.2.4
[0.2.3]: https://github.com/facebookincubator/cargo-guppy/releases/tag/target-spec-0.2.3
[0.2.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/target-spec-0.2.1
[0.2.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/target-spec-0.2.0
[0.1.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/target-spec-0.1.0
