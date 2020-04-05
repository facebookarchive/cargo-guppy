# Changelog

## [0.2.0] - 2020-04-05
### Added
- Added support for parsing specs and platforms separately from evaluating them, making error-less evaluation possible.
- Added support for target features, including situations when target features are unknown.

### Changed
- Switched to [`cfg-expr`](https://github.com/EmbarkStudios/cfg-expr) as the backend for `cfg()` expressions.

## [0.1.0] - 2020-03-20
- Initial release.

[0.2.0]: https://github.com/calibra/cargo-guppy/releases/tag/target-spec-0.2.0
[0.1.0]: https://github.com/calibra/cargo-guppy/releases/tag/target-spec-0.1.0
