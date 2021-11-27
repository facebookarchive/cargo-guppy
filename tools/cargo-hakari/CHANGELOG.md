# Changelog

## [0.9.8] - 2021-11-27

### Added

- Support for [publishing a dummy workspace-hack](https://docs.rs/cargo-hakari/*/cargo_hakari/publishing). This is an
  alternate publishing method that integrates better with existing workflows.
- New config option `dep-format-version`, to control `workspace-hack = ...` lines in other `Cargo.toml`s.
  - Newly initialized workspaces have `dep-format-version = "2"`.
  - Version 2 is required for the alternate publishing method.
  
### Changed

- The default config file location is now `.config/hakari.toml`. `.guppy/hakari.toml` continues to be supported
  as a fallback, so existing users are unaffected.

## [0.9.7] - 2021-11-25

(This release was yanked because it contained a few bugs.)

## [0.9.6] - 2021-10-09

### Fixed

- Backed out the [algorithmic improvement](https://github.com/facebookincubator/cargo-guppy/pull/468) from earlier
  because it didn't handle some edge cases.
- Also simulate builds with dev-dependencies disabled.
- Remove empty sections from the output.

## [0.9.5] - 2021-10-04

### Added

- Support for alternate registries through the `[registries]` section in the config.
  - This is a temporary workaround until [Cargo issue #9052](https://github.com/rust-lang/cargo/issues/9052) is resolved.
- Enable ANSI color output on Windows.

### Fixed

- [Fixed some workspace-hack contents missing in an edge case.](https://github.com/facebookincubator/cargo-guppy/pull/476)

### Optimized

- An [algorithmic improvement](https://github.com/facebookincubator/cargo-guppy/pull/468) in `hakari` makes computation up to 33% faster.

## [0.9.4] - 2021-10-04

### Fixed

- Fixed the configuration example in the readme.

## [0.9.3] - 2021-10-03

### Changed

- The new `"auto"` strategy for the `unify-target-host` option is now the default.
- Updated documentation.

### Fixed

- Fix a rustdoc issue.

## [0.9.2] - 2021-10-01

This was tagged, but never released due to
[docs.rs and rustc nightly issues](https://github.com/rust-lang/docs.rs/issues/1510).

## [0.9.1] - 2021-10-01

### Fixed

- Fix invocation as a cargo plugin.

## [0.9.0] - 2021-10-01

Initial release.

[0.9.8]: https://github.com/facebookincubator/cargo-guppy/releases/tag/cargo-hakari-0.9.8
[0.9.7]: https://github.com/facebookincubator/cargo-guppy/releases/tag/cargo-hakari-0.9.7
[0.9.6]: https://github.com/facebookincubator/cargo-guppy/releases/tag/cargo-hakari-0.9.6
[0.9.5]: https://github.com/facebookincubator/cargo-guppy/releases/tag/cargo-hakari-0.9.5
[0.9.4]: https://github.com/facebookincubator/cargo-guppy/releases/tag/cargo-hakari-0.9.4
[0.9.3]: https://github.com/facebookincubator/cargo-guppy/releases/tag/cargo-hakari-0.9.3
[0.9.2]: https://github.com/facebookincubator/cargo-guppy/releases/tag/cargo-hakari-0.9.2
[0.9.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/cargo-hakari-0.9.1
[0.9.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/cargo-hakari-0.9.0
