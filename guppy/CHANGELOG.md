# Changelog

## Unreleased

### Changed

- MSRV updated to Rust 1.51.

## [0.9.0] - 2021-03-11

### Added

- `DependencyKind::VALUES` lists out all the values of `DependencyKind`.
- `DependencyReq::no_default_features()` returns the enabled status for a dependency when `default-features = false`.

### Changed

- `PackageMetadata::publish` now returns a new, more descriptive `PackagePublish` enum ([#320]).
- `PackageMetadata::readme` now returns `&Utf8Path` rather than `&Path`.
- `BuildTarget::path` now returns `&Utf8Path` rather than `&Path`.

[#320]: https://github.com/facebookincubator/cargo-guppy/issues/320

## [0.8.0] - 2021-02-23

### Changed

- `guppy` now uses [`camino`](https://crates.io/crates/camino) `Utf8Path` and `Utf8PathBuf` wrappers. These wrappers
  provide type-level assertions that returned paths are valid UTF-8.
- Public dependency version bumps:
  - `proptest` updated to version 1 and the corresponding feature renamed to `proptest1`.

## [0.7.2] - 2021-02-15

### Fixed

- Restored compatibility with Rust 1.48. (1.48 is the MSRV, and is now tested in CI.)

## [0.7.1] - 2021-02-14

### Changed

- Packages within a cycle are now returned in non-dev order. When the direction is forward,
  if package Foo has a dependency on Bar, and Bar has a cyclic dev-dependency on
  Foo, then Foo is returned before Bar. (This is not a breaking change because it is an additional
  constraint on guppy itself, not on its consumers.)

## [0.7.0] - 2021-02-03

### Added

* `PackageSource` now has support for parsing external sources through a new `parse_external` method.
* Cargo simulations have some new features:
  * New `CargoOptions::set_initials_platform` method can be used to simulate builds on exclusively the host
    platform.
  * `CargoSet::new` accepts an additional argument, `features_only`, which represents additional inputs that are only
    used for feature unification. This may be used to simulate, e.g. `cargo build --package foo --package bar`, when
    you only care about the results of `foo` but specifying `bar` influences the build.
  * New enum `graph::cargo::BuildPlatform` represents either the target platform or the host. New methods
    `CargoSet::platform_features` and `CargoSet::platform_direct_deps` accept the `BuildPlatform` enum.
* `FeatureSet::contains_package` returns true if a feature set has at least one feature in the given package.
* `semver::VersionReq` is now exposed in `guppy`.
* `FeatureGraph::resolve_ids` resolves feature IDs into a `FeatureSet`.

### Changed

* Feature filters `all_filter`, `default_filter` and `none_filter` have been combined into a single enum
  `StandardFeatures`.
* Cargo builds are now done through `FeatureSet` instances, not `FeatureQuery`. This is because Cargo builds always
  happen in the forward direction.
  * `FeatureQuery::resolve_cargo` has been renamed to `FeatureSet::into_cargo_set`.
* `CargoOptions::with_` methods have been renamed to begin with either `set_` or `add_`.
* `Obs` is now a type rather than a trait.
* `CargoOptions::set_proc_macros_on_target` was replaced with `InitialsPlatform::ProcMacrosOnTarget`.
* Public dependency version bumps:
  * `semver` updated to 0.11.
  * `target-spec` updated to 0.6.

## [0.6.3] - 2021-01-11

### Fixed

* Fix an unintentional use of `serde`'s private exports.

## [0.6.2] - 2020-12-09

### Fixed

* `FeatureGraph::is_default_feature` no longer follows cross-package links.
  
  Cyclic dev-dependencies can enable non-default features (such as testing-only features), and previously
  `is_default_feature` would have returned true for such features. With this change, `is_default_feature`
  returns false for such features.
  
  The `default_filter` feature filter, which uses `is_default_feature`, has been fixed as well.

## [0.6.1] - 2020-12-02

This includes all the changes from version 0.6.0, plus a minor fix:

### Fixed

* Removed "Usage" section from the README, the version number there keeps falling out of sync.

## [0.6.0] - 2020-12-02

(Version 0.6.0 wasn't released to crates.io.)

### Added

* New feature `rayon1`, which introduces support for parallel iterators with [Rayon](https://github.com/rayon-rs/rayon).
  Currently, only a few workspace iterators are supported. More methods will be added as required (if you need
  something, please file an issue or open a PR!)
* `PackageSet` and `FeatureSet` now have `PartialEq` and `Eq` implementations.
  * These implementations check for the graph being same through pointer equality. This means that sets that originate
    from different `PackageGraph` instances will always be unequal, even if they refer to the same packages.
* Added `PackageSet::to_package_query` to convert a `PackageSet` to a `PackageQuery` starting from the same
  elements.

### Changed

* Some methods have been renamed for greater fluency:
  * `FeatureGraph::query_packages` is now `PackageQuery::to_feature_query`.
  * `FeatureGraph::resolve_packages` is now `PackageSet::to_feature_set`.
* The `semver` dependency has been updated to 0.11.

## [0.5.0] - 2020-06-20

This includes the changes in version 0.5.0-rc.1, plus:

### Added

* Support for writing out *build summaries* for `CargoSet` instances through the optional `summaries` feature.

### Changed

* `target-spec` has been upgraded to 0.4.

### Fixed

* `MetadataCommand::exec` and `build_graph` are now `&self`, not `&mut self`.

## [0.5.0-rc.1] - 2020-06-12

### Added

* `PackageGraph::query_workspace_paths` and `resolve_workspace_paths` provide convenient ways
  to create queries and package sets given a list of workspace paths.
* `PackageMetadata::source` provides the source of a package (a local path, `crates.io`, a `git` repository or a custom
  registry).
* `PackageQuery::initials` returns the initial set of packages specified in a package query.
* `FeatureQuery::initials` returns the initial set of features specified in a feature query.
* `FeatureQuery::initial_packages` returns the initial set of *packages* specified in a feature query.
* Improvements to Cargo resolution:
  * `CargoSet` now carries with it the original query and information about
    direct third-party dependencies.
  * A number of bug fixes around edge cases.
* `Workspace::members_by_paths` and `Workspace::members_by_names` look up a list of workspace members
  by path or name, respectively.
* `FeatureGraph::all_features_for` returns a list of all known features for a specified package.

### Changed

* Lookup methods like `PackageGraph::metadata` now return `Result`s with errors instead of `Option`s.
* `target-spec` has been upgraded to 0.3.
* `proptest` has been upgraded to 0.10. The feature has accordingly been renamed to
  `proptest010`.
* `Workspace::members` is now `Workspace::iter_by_path`, and `Workspace::members_by_name` is now `Workspace::iter_by_name`.

### Fixed

* In `FeatureQuery<'g>` and `FeatureSet<'g>`, the lifetime parameter `'g` is now [covariant].
  Compile-time assertions ensure that all lifetime parameters in `guppy` are covariant.

[covariant]: https://github.com/sunshowers/lifetime-variance-example/blob/main/src/lib.rs

### Upcoming

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

[0.9.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-0.9.0
[0.8.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-0.8.0
[0.7.2]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-0.7.2
[0.7.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-0.7.1
[0.7.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-0.7.0
[0.6.3]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-0.6.3
[0.6.2]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-0.6.2
[0.6.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-0.6.1
[0.6.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-0.6.0
[0.5.0]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-0.5.0
[0.5.0-rc.1]: https://github.com/facebookincubator/cargo-guppy/releases/tag/guppy-0.5.0-rc.1
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
