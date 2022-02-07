# Changelog

## Unreleased

## Changed

- Internal dependency version bump: `cfg-expr` updated to 0.10.0.

## [0.9.1] - 2022-02-06

- This is a stub release that re-exports types from version 1.0.

## [0.9.0] - 2021-10-01

## Added

- Target triples can now be parsed directly into a `PlatformSummary`.

## Changed

- `PlatformSummary::new` is now infallible.
- MSRV updated to Rust 1.53.

## Fixed

- `target-spec` now uses `cfg-expr`'s builtins by default, falling back to `target-lexicon` if `cfg-expr` isn't
available.
  - This is because `target-lexicon` [may not always produce results](https://github.com/bytecodealliance/target-lexicon/issues/78)
    that match `rustc`'s target JSONs.

## [0.8.0] - 2021-09-13

### Added

- `Triple` represents a target triple, uniquely identified by a triple string.
- `TargetExpression` represents a target expression beginning with `cfg(`.

### Changed

- `target-spec` now uses [`target-lexicon`](https://github.com/bytecodealliance/target-lexicon) to parse triples,
  while continuing to use `cfg-expr` for expressions and evaluation.
  - Updated supported builtin targets to Rust 1.55.
  - `target-spec` is now more forward compatible, since new targets in future versions of Rust
    can be supported with non-breaking updates to `target-lexicon`.
- `TargetSpec` is now an enum with `Triple` and `TargetExpression` variants.
- `Platform` no longer has a lifetime parameter.
- Updated supported builtin targets to Rust 1.55.
- `cfg-expr` is now a private dependency again (`target-lexicon` is also a private dependency).
- MSRV updated to Rust 1.51.

## [0.7.0] - 2021-02-23

### Changed

- Public dependency version bumps:
  - `cfg-expr` updated to 0.7.1.
  - `proptest` updated to version 1 and the corresponding feature renamed to `proptest1`.

## [0.6.1] - 2021-02-14

### Changed

- `cfg-expr` version requirement relaxed: 0.6 through 0.7 are now supported. There are no API changes between
  the two versions.

## [0.6.0] - 2021-02-03

### Added

- `Platform` now implements `Hash + Eq + Ord`.

### Changed

- `TargetFeatures` and `Platform::add_flags` now accept `Cow<'static, str>`, simplifying lifetime management in many
  cases.
- `cfg-expr` updated to 0.6.0.

## [0.5.0] - 2020-12-02

### Changed

- Updated `cfg-expr` dependency to 0.5.0.

## [0.4.1] - 2020-08-28

### Fixed

- Fixed compilation on platforms without target features ([#175](https://github.com/facebookincubator/cargo-guppy/issues/175)).

## [0.4.0] - 2020-06-20

### Added

- New, optional feature `summaries` to provide serialization and deserialization
  for `Platform` and `TargetFeatures`.
- `Platform::is_custom` returns true if the platform was created with the `custom`
  constructor.
  
### Changed

- The error types have been unified into a single `Error` type.
- `Platform::new` and `Platform::current` now return errors instead of `None`.

## [0.3.0] - 2020-06-12

### Added

- `Platform::custom` creates platforms that are unknown to rustc.
  - This is supported through `cfg-expr`, which is now a public dependency.
  - Custom platforms are often found in embedded Rust.

### Changed

- In order to support custom platforms, `Platform::triple` now returns a `&'a str`
  instead of a `&'static str`.

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

[0.9.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/target-spec-0.9.1
[0.9.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/target-spec-0.9.0
[0.8.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/target-spec-0.8.0
[0.7.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/target-spec-0.7.0
[0.6.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/target-spec-0.6.1
[0.6.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/target-spec-0.6.0
[0.5.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/target-spec-0.5.0
[0.4.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/target-spec-0.4.1
[0.4.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/target-spec-0.4.0
[0.3.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/target-spec-0.3.0
[0.2.4]: https://github.com/facebookincubator/cargo-guppy/releases/tag/target-spec-0.2.4
[0.2.3]: https://github.com/facebookincubator/cargo-guppy/releases/tag/target-spec-0.2.3
[0.2.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/target-spec-0.2.1
[0.2.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/target-spec-0.2.0
[0.1.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/target-spec-0.1.0
