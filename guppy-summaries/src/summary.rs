// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::SummaryDiff;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;
use toml::{Serializer, Value};

/// A type representing a package map as used in `Summary` instances.
pub type PackageMap = BTreeMap<SummaryId, BTreeSet<String>>;

/// A build summary, with the metadata parameter set to the default of `toml::Value`.
pub type Summary = SummaryWithMetadata<Value>;

/// An in-memory representation of a build summary.
///
/// The metadata parameter is customizable.
///
/// For more, see the crate-level documentation.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct SummaryWithMetadata<M = Value> {
    /// Extra metadata associated with the summary.
    ///
    /// This may be used for storing extra information about the summary.
    ///
    /// The type defaults to `toml::Value` but is customizable.
    #[serde(default = "Option::default")]
    pub metadata: Option<M>,

    /// The initial set of packages built on the target platform.
    #[serde(
        rename = "target-initial",
        with = "package_map_impl",
        default = "PackageMap::new"
    )]
    pub target_initials: PackageMap,

    /// The initial set of packages built on the host platform.
    #[serde(
        rename = "host-initial",
        with = "package_map_impl",
        default = "PackageMap::new"
    )]
    pub host_initials: PackageMap,

    /// The packages and features built on the target platform.
    #[serde(
        rename = "target-package",
        with = "package_map_impl",
        default = "PackageMap::new"
    )]
    pub target_packages: PackageMap,

    /// The packages and features built on the host platform.
    #[serde(
        rename = "host-package",
        with = "package_map_impl",
        default = "PackageMap::new"
    )]
    pub host_packages: PackageMap,
}

impl Summary {
    /// Deserializes a summary from the given string.
    ///
    /// This uses the default type `toml::Value` for the metadata. To customize the metadata type,
    /// use `parse_with_metadata`.
    pub fn parse(s: &str) -> Result<Self, toml::de::Error> {
        Self::parse_with_metadata(s)
    }
}

impl<M> SummaryWithMetadata<M> {
    /// Deserializes a summary from the given string, with a custom metadata type parameter.
    pub fn parse_with_metadata<'de>(s: &'de str) -> Result<Self, toml::de::Error>
    where
        M: Deserialize<'de>,
    {
        Ok(toml::from_str(s)?)
    }

    /// Perform a diff of this summary against another.
    ///
    /// This doesn't diff the metadata, just the initials and packages.
    pub fn diff<'a, M2>(&'a self, other: &'a SummaryWithMetadata<M2>) -> SummaryDiff<'a> {
        SummaryDiff::new(self, other)
    }

    /// Serializes this summary to a TOML string.
    pub fn to_string(&self) -> Result<String, toml::ser::Error>
    where
        M: Serialize,
    {
        let mut dst = String::new();
        self.write_to_string(&mut dst)?;
        Ok(dst)
    }

    /// Serializes this summary into the given TOML string, using pretty TOML syntax.
    pub fn write_to_string(&self, dst: &mut String) -> Result<(), toml::ser::Error>
    where
        M: Serialize,
    {
        let mut serializer = Serializer::pretty(dst);
        serializer.pretty_array(false);
        self.serialize(&mut serializer)
    }
}

/// A unique identifier for a package in a build summary.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, Serialize, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub struct SummaryId {
    /// The name of the package.
    pub name: String,

    /// The version number of the package.
    pub version: Version,

    /// The source for this package.
    #[serde(flatten)]
    pub source: SummarySource,
}

impl SummaryId {
    /// Creates a new `SummaryId`.
    pub fn new(name: impl Into<String>, version: Version, source: SummarySource) -> Self {
        Self {
            name: name.into(),
            version,
            source,
        }
    }
}

/// The location of a package.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, Serialize, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case", untagged)]
pub enum SummarySource {
    /// A workspace path.
    Workspace {
        /// The path of this package, relative to the workspace root.
        #[serde(rename = "workspace-path")]
        workspace_path: PathBuf,
    },

    /// A non-workspace path.
    ///
    /// The path is expected to be relative to the workspace root.
    Path {
        /// The path of this package, relative to the workspace root.
        path: PathBuf,
    },

    /// The `crates.io` registry.
    #[serde(with = "crates_io_impl")]
    CratesIo,

    /// An external source that's not the `crates.io` registry, such as an alternate registry or
    /// a `git` repository.
    External {
        /// The external source.
        source: String,
    },
}

impl SummarySource {
    /// Creates a new `SummarySource` representing a workspace source.
    pub fn workspace(workspace_path: impl Into<PathBuf>) -> Self {
        SummarySource::Workspace {
            workspace_path: workspace_path.into(),
        }
    }

    /// Creates a new `SummarySource` representing a non-workspace path source.
    pub fn path(path: impl Into<PathBuf>) -> Self {
        SummarySource::Path { path: path.into() }
    }

    /// Creates a new `SummarySource` representing the `crates.io` registry.
    pub fn crates_io() -> Self {
        SummarySource::CratesIo
    }

    /// Creates a new `SummarySource` representing an external source like a Git repository or a
    /// custom registry.
    pub fn external(source: impl Into<String>) -> Self {
        SummarySource::External {
            source: source.into(),
        }
    }
}

impl fmt::Display for SummarySource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SummarySource::Workspace { workspace_path } => {
                write!(f, "workspace path '{}'", workspace_path.display())
            }
            SummarySource::Path { path } => write!(f, "local path '{}'", path.display()),
            SummarySource::CratesIo => write!(f, "crates.io"),
            SummarySource::External { source } => write!(f, "external '{}'", source),
        }
    }
}

/// Serialization and deserialization for `PackageMap` instances.
mod package_map_impl {
    use super::*;
    use serde::ser::SerializeSeq;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(package_map: &PackageMap, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(package_map.len()))?;
        for (summary_id, features) in package_map {
            seq.serialize_element(&PackageSerialize {
                summary_id,
                features,
            })?;
        }

        seq.end()
    }

    /// TOML representation of a package in a build summary, for serialization.
    #[derive(Serialize)]
    struct PackageSerialize<'a> {
        #[serde(flatten)]
        summary_id: &'a SummaryId,
        features: &'a BTreeSet<String>,
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<PackageMap, D::Error>
    where
        D: Deserializer<'de>,
    {
        let packages = Vec::<PackageDeserialize>::deserialize(deserializer)?;
        let mut package_map: PackageMap = BTreeMap::new();

        for package in packages {
            package_map.insert(package.summary_id, package.features);
        }
        Ok(package_map)
    }

    /// TOML representation of a package in a build summary, for deserialization.
    #[derive(Deserialize)]
    struct PackageDeserialize {
        #[serde(flatten)]
        summary_id: SummaryId,
        features: BTreeSet<String>,
    }
}

/// Serialization and deserialization for the `CratesIo` variant.
mod crates_io_impl {
    use super::*;
    use serde::de::Error;
    use serde::ser::SerializeMap;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("crates-io", &true)?;
        map.end()
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        let crates_io = CratesIoDeserialize::deserialize(deserializer)?;
        if !crates_io.crates_io {
            return Err(D::Error::custom("crates-io field should be true"));
        }
        Ok(())
    }

    #[derive(Deserialize)]
    struct CratesIoDeserialize {
        #[serde(rename = "crates-io")]
        crates_io: bool,
    }
}
