# hakari

[![hakari on crates.io](https://img.shields.io/crates/v/hakari)](https://crates.io/crates/hakari) [![Documentation (latest release)](https://docs.rs/hakari/badge.svg)](https://docs.rs/hakari/) [![Documentation (master)](https://img.shields.io/badge/docs-master-brightgreen)](https://facebookincubator.github.io/cargo-guppy/rustdoc/hakari/) [![License](https://img.shields.io/badge/license-Apache-green.svg)](../../LICENSE-APACHE) [![License](https://img.shields.io/badge/license-MIT-green.svg)](../../LICENSE-MIT)

`hakari` is a set of tools to manage `workspace-hack` packages.

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

These packages have historically been maintained by hand, on a best-effort basis.

## What hakari does

`hakari` is a set of tools to automate the management of these `workspace-hack` packages.

TODO: write up how it works.

## TODOs

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
