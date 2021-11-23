# Changelog

## [0.7.0] - 2021-11-23

### Fixed

- Updated example in `README.md` now that the `cargo-guppy` repository has a workspace-hack.

### Changed

- Updated to `guppy 0.12.0`.

## [0.6.2] - 2021-10-09

### Fixed

- Backed out the [algorithmic improvement](https://github.com/facebookincubator/cargo-guppy/pull/468) from earlier
  because it didn't handle some edge cases.
- Also simulate builds with dev-dependencies disabled.
- Remove empty sections from the output.

## [0.6.1] - 2021-10-06

### Added

- Support for alternate registries through the `[registries]` section in the config.
  - This is a temporary workaround until [Cargo issue #9052](https://github.com/rust-lang/cargo/issues/9052) is resolved.

### Fixed

- [Fixed some workspace-hack contents missing in an edge case.](https://github.com/facebookincubator/cargo-guppy/pull/476)

### Optimized

- An [algorithmic improvement](https://github.com/facebookincubator/cargo-guppy/pull/468) makes computation up to 33% faster.

## [0.6.0] - 2021-10-03

### Added

- A new `UnifyTargetHost::Auto` strategy, which uses the `ReplicateTargetOnHost` strategy
  if there are internal build dependencies or proc macros in the workspace, or the `UnifyIfBoth` strategy
  if not.

### Changed

- For `UnifyTargetHost`:
  - `Auto` is the new default strategy.
  - `ReplicateTargetAsHost` has been renamed to `ReplicateTargetOnHost`.
  - `UnifyOnBoth` has been renamed to `UnifyIfBoth`.

### Fixed

- Fixed some formatting issues with `WorkspaceOps`.

## [0.5.0] - 2021-10-01

### Added

- hakari now outputs packages corresponding to the intersection of all platforms, then outputs
  any other platform-specific packages left. This simplifies the output greatly and is also more
  correct.
- A new option `final_excludes` to remove packages from the result at the end of computation.
  - This is in constract to `traversal_excludes` (renamed from `omitted_packages`) which removes
    packages both during and after computation.
- A new `cli-support` feature contains several new structs used by `cargo-hakari`.
- In `HakariBuilderSummary`, `version = "v2"` etc has been renamed to `resolver = "2"` to align with
  cargo.
  - The old options will continue to work.
- In `HakariBuilderSummary`, `traversal-excludes` and `final-excludes` are now easier to describe while
  deserializing: they now take a `workspace-members` list of names, and a `third-party` list of specifiers such as
  `{ name = "serde", version = "1" }`.
  - The resolver will now also fail if any specifiers are unmatched.

### Changed

- `omitted_packages` renamed to `traversal_excludes`.
- Because of the changes to how excludes are represented, old-style `HakariBuilderSummary` instances
  may no longer parse correctly.
- Public dependency bump: `guppy` updated to 0.11.1.
- MSRV updated to Rust 1.53.

## [0.4.1] - 2021-09-13

### Changed

- Public dependency version bump: `guppy` updated to 0.10.1.
- MSRV updated to Rust 1.51.

## [0.4.0] - 2021-09-13

(This release was never published because it was based on `guppy 0.10.0`, which was yanked.)

## [0.3.0] - 2021-03-11

### Changed

- Public dependency version bump: `guppy` updated to 0.9.0.
- `HakariCargoToml` now uses `camino`'s UTF-8 paths.
  - `HakariCargoToml::new` now accepts `impl Into<Utf8PathBuf>` rather than `impl Into<PathBuf>`.
  - `HakariCargoToml::toml_path` returns `&Utf8Path` instead of `&Path`.

## [0.2.0] - 2021-02-23

### Changed

- `hakari` now uses [`camino`](https://crates.io/crates/camino) `Utf8Path` and `Utf8PathBuf` wrappers. These wrappers
  provide type-level assertions that returned paths are valid UTF-8.
- Public dependency version bumps:
  - `proptest` updated to version 1 and the corresponding feature renamed to `proptest1`.

## [0.1.1] - 2021-02-04

### Added

* Experimental Windows support. There may still be bugs around path normalization: please [report them](https://github.com/facebookincubator/cargo-guppy/issues/new).

### Fixed

* Fixed Cargo.toml output for path dependencies.
* Return an error for non-Unicode paths instead of silently producing incorrect paths.

## [0.1.0] - 2021-02-03

Initial release.

[0.7.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/hakari-0.7.0
[0.6.2]: https://github.com/facebookincubator/cargo-guppy/releases/tag/hakari-0.6.2
[0.6.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/hakari-0.6.1
[0.6.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/hakari-0.6.0
[0.5.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/hakari-0.5.0
[0.4.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/hakari-0.4.1
[0.4.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/hakari-0.4.0
[0.3.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/hakari-0.3.0
[0.2.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/hakari-0.2.0
[0.1.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/hakari-0.1.1
[0.1.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/hakari-0.1.0
