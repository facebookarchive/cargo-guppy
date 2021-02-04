# hakari

[![hakari on crates.io](https://img.shields.io/crates/v/hakari)](https://crates.io/crates/hakari) [![Documentation (latest release)](https://docs.rs/hakari/badge.svg)](https://docs.rs/hakari/) [![Documentation (main)](https://img.shields.io/badge/docs-main-brightgreen)](https://facebookincubator.github.io/cargo-guppy/rustdoc/hakari/) [![License](https://img.shields.io/badge/license-Apache-green.svg)](../../LICENSE-APACHE) [![License](https://img.shields.io/badge/license-MIT-green.svg)](../../LICENSE-MIT)

`hakari` is a set of tools to manage `workspace-hack` packages.

## Examples

```rust
use guppy::MetadataCommand;
use hakari::{HakariBuilder, TomlOptions};

// Use this workspace's PackageGraph for these tests.
let package_graph = MetadataCommand::new()
    .build_graph()
    .expect("obtained cargo-guppy's PackageGraph");
// The second argument to HakariBuilder::new specifies a Hakari (workspace-hack) package. At
// the moment cargo-guppy does not have such a package, and it is a TODO to add one.
let hakari_builder = HakariBuilder::new(&package_graph, None)
    .expect("HakariBuilder was constructed");

// HakariBuilder has a number of config options. For this example, use the defaults.
let hakari = hakari_builder.compute();

// "hakari" can be used to build a TOML representation that forms part of a Cargo.toml file.
// Existing Cargo.toml files can be managed using Hakari::read_toml.
let toml = hakari.to_toml_string(&TomlOptions::default()).expect("TOML output was constructed");

// toml contains the Cargo.toml [dependencies] that would go in the Hakari package. It can be
// written out through `HakariCargoToml` (returned by Hakari::read_toml) or manually.
println!("Cargo.toml contents:\n{}", toml);
```

The `cargo-guppy` repository also has a number of fixtures that demonstrate Hakari's output.
[Here is an example](https://github.com/facebookincubator/cargo-guppy/blob/main/fixtures/guppy/hakari/metadata_guppy_869476c-1.toml).

## Platform support

* **Unix platforms**: Hakari works and is supported.
* **Windows**: Hakari works and outputs file paths with forward slashes for
  consistency with Unix. CRLF line endings are not supported in the workspace-hack's
  `Cargo.toml` -- it is recommended that repositories disable automatic line ending conversion.
  [Here's how to do it in Git](https://stackoverflow.com/a/10017566).
  (Pull requests to improve this are welcome.)

## What are workspace-hack packages?

Let's say you have a Rust crate `my-crate` with two dependencies:

```toml
# my-crate/Cargo.toml
[dependencies]
foo = "1.0"
bar = "2.0"
```

Let's say that `foo` and `bar` both depend on `baz`:

```toml
# foo-1.0/Cargo.toml
[dependencies]
baz = { version = "1", features = ["a", "b"] }

# bar-2.0/Cargo.toml
[dependencies]
baz = { version = "1", features = ["b", "c"] }
```

What features is `baz` built with?

One way to resolve this question might be to build `baz` twice with each requested set of
features. But this is likely to cause a combinatorial explosion of crates to build, so Cargo
doesn't do that. Instead,
[Cargo builds `baz` once](https://doc.rust-lang.org/nightly/cargo/reference/features.html?highlight=feature#feature-unification)
with the *union* of the features enabled for the package: `[a, b, c]`.

---

**NOTE:** This description elides some details around unifying build and dev-dependencies: for
more about this, see the documentation for guppy's
[`CargoResolverVersion`](guppy::graph::cargo::CargoResolverVersion).

---

Now let's say you're in a workspace, with a second crate `your-crate`:

```toml
# your-crate/Cargo.toml
[dependencies]
baz = { version = "1", features = ["c", "d"] }
```

In this situation:

| if you build                                 | `baz` is built with |
| -------------------------------------------- | ------------------- |
| just `my-crate`                              | `a, b, c`           |
| just `your-crate`                            | `c, d`              |
| `my-crate` and `your-crate` at the same time | `a, b, c, d`        |

Even in this simplified scenario, there are three separate ways to build `baz`. For a dependency
like [`syn`](https://crates.io/crates/syn) that have
[many optional features](https://github.com/dtolnay/syn#optional-features),
large workspaces end up with a very large number of possible build configurations.

Even worse, the feature set of a package affects everything that depends on it, so `syn`
being built with a slightly different feature set than before would cause *every package that
directly or transitively depends on `syn` to be rebuilt. For large workspaces, this can result
a lot of wasted build time.

---

To avoid this problem, many large workspaces contain a `workspace-hack` package. The
purpose of this package is to ensure that dependencies like `syn` are always built with the same
feature set no matter which workspace packages are currently being built. This is done by:
1. adding dependencies like `syn` to `workspace-hack` with the full feature set required by any
  package in the workspace
2. adding `workspace-hack` as a dependency of every crate in the repository.

Some examples of `workspace-hack` packages:

* Rust's [`rustc-workspace-hack`](https://github.com/rust-lang/rust/blob/0bfc45aa859b94cedeffcbd949f9aaad9f3ac8d8/src/tools/rustc-workspace-hack/Cargo.toml)
* Firefox's [`mozilla-central-workspace-hack`](https://hg.mozilla.org/mozilla-central/file/cf6956a5ec8e21896736f96237b1476c9d0aaf45/build/workspace-hack/Cargo.toml)
* Diem's [`diem-workspace-hack`](https://github.com/diem/diem/blob/91578fec8d575294b47b3ee7af691fd9dc6eb240/common/workspace-hack/Cargo.toml)

These packages have historically been maintained by hand, on a best-effort basis. `hakari` is an
attempt to automate the maintenance of these packages.

## How `hakari` works

Hakari follows a three-step process.

### 1. Configuration

A [`HakariBuilder`](HakariBuilder) provides options to configure how a Hakari computation is done. Options supported
include:
* [the location of the `workspace-hack` package](HakariBuilder::new)
* [platforms to simulate Cargo builds on](HakariBuilder::set_platforms)
* [the version of the Cargo resolver to use](HakariBuilder::set_resolver_version)
* [packages to be omitted from the computation](HakariBuilder::add_omitted_packages)
* [a "verify mode" to ensure that dependency feature sets are correctly unified](HakariBuilder::set_verify_mode)

With the optional `summaries` feature, `HakariBuilder` options can be
[read from](HakariBuilder::from_summary) or [written to](HakariBuilder::to_summary)
a file as TOML or some other format.

### 2. Computation

Once a `HakariBuilder` is configured, its [`compute`](HakariBuilder::compute) method can be
called to create a `Hakari` instance. The algorithm runs in three steps:

1. Use guppy to [simulate a Cargo build](guppy::graph::cargo) for every workspace package and
   every given platform, with no features, default features and all features. Collect the
   results into
   [a map](internals::ComputedMap) indexed by every dependency and the different sets of
   features it was built with.
2. Scan through the map to figure out which dependencies are built with two or more
   different feature sets, collecting them into an [output map](internals::OutputMap).
3. If one assumes that the output map will be written out to the `workspace-hack` package
   through step 3 below, it is possible that it causes some extra packages to be built with a
   second feature set. Look for such packages, add them to the output map, and iterate until a
   fixpoint is reached and no new packages are built more than one way.

This computation is done in a parallel fashion, using the [Rayon](rayon) library.

The result of this computation is a [`Hakari`](Hakari) instance.

### 3. Serialization

The last step is to serialize the contents of the output map into the `workspace-hack` package's
`Cargo.toml` file.

1. [`Hakari::read_toml`] reads an existing `Cargo.toml` file on disk. This file is
   *partially generated*:

   ```toml
   [package]
   name = "workspace-hack"
   version = "0.1.0"
   # more options...

   ### BEGIN HAKARI SECTION
   ...
   ### END HAKARI SECTION
   ```

   The contents outside the `BEGIN HAKARI SECTION` and `END HAKARI SECTION` lines may be
   edited by hand. The contents within this section are automatically generated.

   On success, a [`HakariCargoToml`](HakariCargoToml) is returned.

2. [`Hakari::to_toml_string`](Hakari::to_toml_string) returns the new contents of the
   automatically generated section.
3. [`HakariCargoToml::write_to_file`](HakariCargoToml::write_to_file) writes out the contents
   to disk.

`HakariCargoToml` also supports serializing contents to memory and producing diffs.

## Future work

`hakari` is a work-in-progress and is still missing many core features:
* Simulating cross-compilations
* Omitting some packages on some environments
* Excluding some packages from the final result
* Only including a subset of packages in the final result (e.g. unifying core packages like
  `syn` but not any others)
* Automating the creation of `workspace-hack` packages
* Support for alternate registries (depends on
  [Cargo issue #9052](https://github.com/rust-lang/cargo/issues/9052))
* A command-line interface

These features will be added as time permits.

## Contributing

See the [CONTRIBUTING](../../CONTRIBUTING.md) file for how to help out.

## License

This project is available under the terms of either the [Apache 2.0 license](../../LICENSE-APACHE) or the [MIT
license](../../LICENSE-MIT).

<!--
README.md is generated from README.tpl by cargo readme. To regenerate:

cargo install cargo-readme
cargo readme > README.md
-->
