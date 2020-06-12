# Changelog

## Unreleased

### Added

* `PackageMetadata::source` provides the source of a package (a local path, `crates.io`, a `git` repository or a custom
  registry).
* Improvements to Cargo resolution:
  * Fine-grained control over Cargo resolution with postfilters.
  * `CargoSet` now carries with it the original query and information about
    direct third-party dependencies.
  * A number of bug fixes around edge cases. 

### Changed

* `CargoOptions` now needs to be passed in as `&mut` rather than `&`.
* `target-spec` has been upgraded to 0.3.
* `proptest` has been upgraded to 0.10. The feature has accordingly been renamed to
  `proptest010`.
  
## Upcoming

* Support for *build summaries* is currently in an experimental state.

## [0.4.1] - 2020-05-07

This is a small followup release with some APIs that were meant to be added to 0.4.0.

### Added
- `PackageGraph` now has some new `resolve_` methods:
  - `resolve_ids`: creates a `PackageSet` with the specified package IDs.
  - `resolve_workspace`: creates a `PackageSet` with all workspace packages (but no transitive dependencies).
  - `resolve_workspace_names`: creates a `PackageSet` with the specified workspace packages by name (but no transitive
    dependencies).

## [0.4.0] - 2020-05-06

This is a major overhaul of `guppy`, with many new features and several changed APIs.

### Added
- Support for graph analysis on a per-feature basis.
  - The APIs are contained in `guppy::graph::feature`, and are accessible through `PackageGraph::feature_graph`.
  - An almost complete set of queries and operations is available through `FeatureQuery` and `FeatureSet`.
- Support for simulating what packages and features would be built by Cargo.
  - The APIs are contained in `guppy::graph::cargo`, and are accessible by constructing a `FeatureQuery` and using its
    `resolve_cargo` method.
  - Both the current resolver and the upcoming [V2 resolver](https://github.com/rust-lang/cargo/pull/7820) are
    supported, and there are extensive property-based tests to ensure that `guppy` faithfully emulates `cargo`.
- `PackageQuery` (and `FeatureQuery`) can now be introspected with new methods `direction` and `starts_from`.
- `PackageMetadata` instances now have `has_build_script` and `is_proc_macro` methods.
- Add `PackageGraph::query_workspace_names` to make a `PackageQuery` by workspace name.

### Changed
- `PackageSet`'s consuming `into_` iterators have been turned into borrowing iterators.
  - `into_ids` is now `ids`, and `into_links` is now `links`.
- Direct dependency and reverse dependency queries now live on `PackageMetadata` instances.
- `PackageLink`, instead of having public `from`, `to` and `edge` fields, now has methods which return that data.
  - The functionality of `PackageEdge` has been subsumed into `PackageLink`.
- The data model for platform-specific statuses has been overhauled. See `EnabledStatus`, `PlatformStatus` and
  `PlatformEval`.
- `PackageResolver` (and `FeatureResolver`) improvements.
  - Resolver instances now have the query passed in, to make it easier to write stateless resolvers.
  - Resolver instances now take in `&mut self` instead of a plain `&self` (or `FnMut` instead of `Fn`).
- `MetadataCommand` has been reimplemented in `guppy`, and now has a `build_graph` method.
  - `Metadata` has been reworked as well, and renamed to `CargoMetadata`.

### Removed
- `PackageGraph::retain_edges` no longer exists: its functionality can be replicated through `PackageResolver`.

## [0.3.1] - 2020-04-15

### Added
- Support for listing and querying build targets (library, binaries, tests, etc) within a package.
  - `PackageMetadata::build_targets`: iterates over all build targets within a package.
  - `PackageMetadata::build_target`: retrieves a build target by identifier.

## [0.3.0] - 2020-04-14

This is a breaking release with some minor API changes.

### Added
- `PackageGraph::directly_depends_on`: returns true if a package directly depends on another.
- `Workspace` has new `member_by_name` and `members_by_name` methods for workspace lookups by name.

### Fixed
- `guppy` now checks for duplicate names in workspaces and errors out if it finds any.

### Changed
- `Workspace::members` and `Workspace::member_by_path` now return `PackageMetadata` instances, not `PackageId`.

## [0.2.1] - 2020-04-13

### Fixed
- Fixed a build issue on nightly Rust.

## [0.2.0] - 2020-04-13

This is a breaking release. There are no new or removed features, but many existing APIs have been cleaned up.

### Changed
- The `select_` methods have been renamed to `query_`.
  - `PackageSelect` is now `PackageQuery`.
- `select_all` is now `resolve_all` and directly produces a `PackageSet`.
- `DependencyLink` is now `PackageLink`, and `DependencyEdge` is now `PackageEdge`.
- `into_iter_links` is now `PackageSet::into_links`.
- `PackageId` is now custom to `guppy` instead of reusing `cargo_metadata::PackageId`.
- `PackageDotVisitor` now takes a `&mut DotWrite`.

### Removed
- All previously deprecated methods have been cleaned up.

## [0.1.8] - 2020-04-08
### Added
- Implemented package resolution using custom resolvers, represented by the `PackageResolver` trait.
  - Added new APIs `PackageSelect::resolve_with` and `PackageSelect::resolve_with_fn`.
  - A `PackageResolver` provides fine-grained control over which links are followed.
  - It is equivalent to `PackageGraph::retain_edges`, but doesn't borrow mutably and is scoped to a single selector.
- Added `PackageSet` to represent a set of known, resolved packages.
  - `PackageSet` comes with the standard set operations: `len`, `contains`, `union`, `intersection`, `difference` and
    `symmetric_difference`.
  - A `PackageSet` can also be iterated on in various ways, listed in the "Deprecated" section.

### Changed
- Updated repository links.

### Deprecated
- The following `into_` methods on `PackageSelect` have been deprecated and moved to `PackageSet`.
  - `select.into_iter_ids()` -> `select.resolve().into_ids()`
  - `select.into_iter_metadatas()` -> `select.resolve().into_metadatas()`
  - `select.into_root_ids()` -> `select.resolve().into_root_ids()`
  - `select.into_root_metadatas()` -> `select.resolve().into_root_metadatas()`

## [0.1.7] - 2020-04-05
### Added
- Support for [platform-specific dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#platform-specific-dependencies), including:
   - Querying whether a dependency is required or optional on the current platform, or on any other platform.
   - Evaluating which features are enabled on a platform.
   - Handling situations where the set of [target features](https://github.com/rust-lang/rfcs/blob/master/text/2045-target-feature.md) isn't known.

### Changed
- Internal improvements -- `into_iter_ids` is a further 10-15% faster for large graphs.
- Made several internal changes to prepare for feature graph support, coming soon.
- Sped up build times by removing some dependencies.

### Deprecated
- As part of support for platform-specific dependencies, `DependencyMetadata::target` has been replaced with the `_on` methods.
  - For example, to figure out if a dependency is enabled on a platform, use the `enabled_on` method.

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
- New optional feature `proptest010` to help with property testing.

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

[0.4.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-0.4.1
[0.4.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-0.4.0
[0.3.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-0.3.1
[0.3.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-0.3.0
[0.2.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-0.2.1
[0.2.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-0.2.0
[0.1.8]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-0.1.8
[0.1.7]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-0.1.7

<!-- Previous releases were simply tagged "$VERSION", not "guppy-$VERSION". -->
[0.1.6]: https://github.com/facebookincubator/cargo-guppy/releases/tag/0.1.6
[0.1.5]: https://github.com/facebookincubator/cargo-guppy/releases/tag/0.1.5
[0.1.4]: https://github.com/facebookincubator/cargo-guppy/releases/tag/0.1.4
[0.1.3]: https://github.com/facebookincubator/cargo-guppy/releases/tag/0.1.3
[0.1.2]: https://github.com/facebookincubator/cargo-guppy/releases/tag/0.1.2
[0.1.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/0.1.1
[0.1.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/0.1.0
