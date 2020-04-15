// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::borrow::Borrow;
use std::cmp::Ordering;
use std::path::Path;

/// A build target in a package.
///
/// A build target consists of one or more source files which can be compiled into a crate.
///
/// For more, see [Cargo
/// Targets](https://doc.rust-lang.org/nightly/cargo/reference/cargo-targets.html) in the Cargo
/// reference.
pub struct BuildTarget<'g> {
    id: BuildTargetId<'g>,
    inner: &'g BuildTargetImpl,
}

impl<'g> BuildTarget<'g> {
    // The weird function signature is so that .map(BuildTarget::new) can be called.
    pub(super) fn new((id, inner): (&'g OwnedBuildTargetId, &'g BuildTargetImpl)) -> Self {
        Self {
            id: id.as_borrowed(),
            inner,
        }
    }

    /// Returns the unique identifier for this build target.
    pub fn id(&self) -> BuildTargetId<'g> {
        self.id
    }

    /// Returns the name of this build target.
    pub fn name(&self) -> &'g str {
        match self.id {
            BuildTargetId::Library | BuildTargetId::BuildScript => self
                .inner
                .lib_name
                .as_ref()
                .expect("library targets have lib_name set"),
            other => other.name().expect("non-library targets can't return None"),
        }
    }

    /// Returns the kind of this build target.
    pub fn kind(&self) -> &'g BuildTargetKind {
        &self.inner.kind
    }

    /// Returns the features required for this build target.
    ///
    /// This setting has no effect on the library target.
    ///
    /// For more, see [The `required-features`
    /// field](https://doc.rust-lang.org/nightly/cargo/reference/cargo-targets.html#the-required-features-field)
    /// in the Cargo reference.
    pub fn required_features(&self) -> &'g [String] {
        &self.inner.required_features
    }

    /// Returns the absolute path of the location where the source for this build target is located.
    pub fn path(&self) -> &'g Path {
        &self.inner.path
    }

    /// Returns the Rust edition for this build target.
    pub fn edition(&self) -> &'g str {
        &self.inner.edition
    }

    /// Returns true if documentation tests are enabled for this build target.
    pub fn doc_tests(&self) -> bool {
        self.inner.doc_tests
    }
}

/// An identifier for a build target within a package.
#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum BuildTargetId<'g> {
    /// A library target.
    ///
    /// There may be at most one of these in a package.
    ///
    /// Defined by the `[lib]` section in `Cargo.toml`.
    Library,
    /// A build script.
    ///
    /// There may be at most one of these in a package.
    ///
    /// Defined by the `build` attribute in `Cargo.toml`. For more about build scripts, see [Build
    /// Scripts](https://doc.rust-lang.org/nightly/cargo/reference/build-scripts.html) in the Cargo
    /// reference.
    BuildScript,
    /// A binary target with its name.
    ///
    /// Defined by the `[[bin]]` section in `Cargo.toml`.
    Binary(&'g str),
    /// An example target with its name.
    ///
    /// Examples are typically binary, but may be libraries or even both.
    ///
    /// Defined by the `[[example]]` section in `Cargo.toml`.
    Example(&'g str),
    /// A test target with its name.
    ///
    /// Tests are always binary targets.
    ///
    /// Defined by the `[[test]]` section in `Cargo.toml`.
    Test(&'g str),
    /// A benchmark target with its name.
    ///
    /// Benchmarks are always binary targets.
    ///
    /// Defined by the `[[bench]]` section in `Cargo.toml`.
    Benchmark(&'g str),
}

impl<'g> BuildTargetId<'g> {
    /// Returns the name embedded in this identifier, or `None` if this is a library target.
    ///
    /// To get the name of the library target, use `BuildTarget::name`.
    pub fn name(&self) -> Option<&'g str> {
        match self {
            BuildTargetId::Library => None,
            BuildTargetId::BuildScript => None,
            BuildTargetId::Binary(name) => Some(name),
            BuildTargetId::Example(name) => Some(name),
            BuildTargetId::Test(name) => Some(name),
            BuildTargetId::Benchmark(name) => Some(name),
        }
    }

    pub(super) fn as_key(&self) -> &(dyn BuildTargetKey + 'g) {
        self
    }
}

/// The type of build target (library or binary).
///
/// Obtained through `BuildTarget::kind`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum BuildTargetKind {
    /// This build target is a library or example, with the specified crate types.
    ///
    /// Note that examples are typically binaries, but they may be libraries as well. Binary
    /// examples will have the crate type `"bin"`.
    ///
    /// For more about crate types, see [The `crate-type`
    /// field](https://doc.rust-lang.org/nightly/cargo/reference/cargo-targets.html#the-crate-type-field)
    /// in the Cargo reference.
    LibraryOrExample(Vec<String>),
    /// This build target is a binary target.
    ///
    /// This kind is returned for build script, binary, test, and benchmark targets.
    Binary,
}

/// Stored data in a `BuildTarget`.
#[derive(Clone, Debug)]
pub(super) struct BuildTargetImpl {
    pub(super) kind: BuildTargetKind,
    // This is only set if the id is BuildTargetId::Library.
    pub(super) lib_name: Option<Box<str>>,
    pub(super) required_features: Vec<String>,
    pub(super) path: Box<Path>,
    pub(super) edition: Box<str>,
    pub(super) doc_tests: bool,
}

/// Owned version of `BuildTargetId`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(all(test, feature = "proptest09"), derive(proptest_derive::Arbitrary))]
pub(super) enum OwnedBuildTargetId {
    Library,
    BuildScript,
    Binary(Box<str>),
    Example(Box<str>),
    Test(Box<str>),
    Benchmark(Box<str>),
}

impl OwnedBuildTargetId {
    fn as_borrowed(&self) -> BuildTargetId {
        match self {
            OwnedBuildTargetId::Library => BuildTargetId::Library,
            OwnedBuildTargetId::BuildScript => BuildTargetId::BuildScript,
            OwnedBuildTargetId::Binary(name) => BuildTargetId::Binary(name.as_ref()),
            OwnedBuildTargetId::Example(name) => BuildTargetId::Example(name.as_ref()),
            OwnedBuildTargetId::Test(name) => BuildTargetId::Test(name.as_ref()),
            OwnedBuildTargetId::Benchmark(name) => BuildTargetId::Benchmark(name.as_ref()),
        }
    }
}

// Allow Borrow usage for complex keys. Adapted from
// http://idubrov.name/rust/2018/06/01/tricking-the-hashmap.html.
pub(super) trait BuildTargetKey {
    fn key(&self) -> BuildTargetId;
}

impl<'g> BuildTargetKey for BuildTargetId<'g> {
    fn key(&self) -> BuildTargetId {
        *self
    }
}

impl BuildTargetKey for OwnedBuildTargetId {
    fn key(&self) -> BuildTargetId {
        self.as_borrowed()
    }
}

impl<'g> Borrow<dyn BuildTargetKey + 'g> for OwnedBuildTargetId {
    fn borrow(&self) -> &(dyn BuildTargetKey + 'g) {
        self
    }
}

impl<'g> PartialEq for (dyn BuildTargetKey + 'g) {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl<'g> Eq for (dyn BuildTargetKey + 'g) {}

// For Borrow to be upheld, PartialOrd and Ord should be consistent. This is checked by the proptest
// below.
impl<'g> PartialOrd for (dyn BuildTargetKey + 'g) {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.key().partial_cmp(&other.key())
    }
}

impl<'g> Ord for (dyn BuildTargetKey + 'g) {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key().cmp(&other.key())
    }
}

#[cfg(all(test, feature = "proptest09"))]
mod tests {
    use super::*;
    use proptest::prelude::*;

    impl OwnedBuildTargetId {
        fn as_key(&self) -> &dyn BuildTargetKey {
            self
        }
    }

    proptest! {
        #[test]
        fn consistent_borrow(id1 in any::<OwnedBuildTargetId>(), id2 in any::<OwnedBuildTargetId>()) {
            prop_assert_eq!(
                id1.eq(&id1),
                id1.as_key().eq(id1.as_key()),
                "consistent eq implementation (same IDs)"
            );
            prop_assert_eq!(
                id1.eq(&id2),
                id1.as_key().eq(id2.as_key()),
                "consistent eq implementation (different IDs)"
            );
            prop_assert_eq!(
                id1.partial_cmp(&id2),
                id1.as_key().partial_cmp(id2.as_key()),
                "consistent partial_cmp implementation"
            );
            prop_assert_eq!(
                id1.cmp(&id2),
                id1.as_key().cmp(id2.as_key()),
                "consistent cmp implementation"
            );
        }
    }
}
