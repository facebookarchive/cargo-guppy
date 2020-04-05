# Changelog

## [0.1.6] - 2020-03-11
### Fixed
- Handle cyclic dev-dependencies properly. Previously, `guppy` could produce incomplete results if it encountered cycles.

### Changed
- As a result of algorithmic improvements to handle cycles, `into_iter_ids` is now around 60% faster for large graphs.

## [0.1.5] - 2020-03-06
### Fixed
- Fix a bug involving situations where different dependency sections depend on the same package with different versions:

```toml
[dependencies]
lazy_static = "1"

[dev-dependencies]
lazy_static = "0.2"
```

## [0.1.4] - 2020-01-26
### Added
- New selector `select_workspace` to select packages that are part of the workspace and all their transitive
  dependencies. In general, `select_workspace` is preferable over `select_all`.

### Fixed
- Fixed a bug in `into_root_ids` and `into_root_metadatas` that would cause it to return packages that aren't roots of
  another package.

### Changed
- Internal upgrades to prepare for upcoming feature graph analysis.

## [0.1.3] - 2019-12-29
### Added
- `PackageSelect::into_root_metadatas` returns package metadatas for all roots within a selection.
- New optional feature `proptest09` to help with property testing.

### Changed
- Upgrade to `petgraph` 0.5 -- this allows for some internal code to be simplified.

### Deprecated
- Package selectors have been renamed. The old names will continue to work for the 0.1 series, but will be removed in the 0.2 series.
  - `select_transitive_deps` → `select_forward`
  - `select_reverse_transitive_deps` → `select_reverse`
  - `select_transitive_deps_directed` → `select_directed`

## [0.1.2] - 2019-11-26
### Fixed
- Fixed the return type of `into_root_ids` to be `impl Iterator` instead of `impl IntoIterator`.

## [0.1.1] - 2019-11-22
### Fixed
- Fixed a publishing issue with version 0.1.0.

## [0.1.0] - 2019-11-22
### Added
- Initial release.

<!-- Previous releases were simply tagged "$VERSION", not "guppy-$VERSION". -->

[0.1.6]: https://github.com/calibra/cargo-guppy/releases/tag/0.1.6
[0.1.5]: https://github.com/calibra/cargo-guppy/releases/tag/0.1.5
[0.1.4]: https://github.com/calibra/cargo-guppy/releases/tag/0.1.4
[0.1.3]: https://github.com/calibra/cargo-guppy/releases/tag/0.1.3
[0.1.2]: https://github.com/calibra/cargo-guppy/releases/tag/0.1.2
[0.1.1]: https://github.com/calibra/cargo-guppy/releases/tag/0.1.1
[0.1.0]: https://github.com/calibra/cargo-guppy/releases/tag/0.1.0
