// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::diff::SummaryDiff;
use camino::Utf8PathBuf;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};
use toml::{Serializer, Value};

/// A type representing a package map as used in `Summary` instances.
pub type PackageMap = BTreeMap<SummaryId, PackageInfo>;

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

    /// The packages and features built on the target platform.
    #[serde(
        rename = "target-package",
        with = "package_map_impl",
        default = "PackageMap::new",
        skip_serializing_if = "PackageMap::is_empty"
    )]
    pub target_packages: PackageMap,

    /// The packages and features built on the host platform.
    #[serde(
        rename = "host-package",
        with = "package_map_impl",
        default = "PackageMap::new",
        skip_serializing_if = "PackageMap::is_empty"
    )]
    pub host_packages: PackageMap,
}

impl<M> SummaryWithMetadata<M> {
    /// Deserializes a summary from the given string, with optional custom metadata.
    pub fn parse<'de>(s: &'de str) -> Result<Self, toml::de::Error>
    where
        M: Deserialize<'de>,
    {
        toml::from_str(s)
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

impl<M> Default for SummaryWithMetadata<M> {
    fn default() -> Self {
        Self {
            metadata: None,
            target_packages: PackageMap::new(),
            host_packages: PackageMap::new(),
        }
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

impl fmt::Display for SummaryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{ name = \"{}\", version = \"{}\", source = \"{}\"}}",
            self.name, self.version, self.source
        )
    }
}

/// The location of a package.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, Serialize, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case", untagged)]
pub enum SummarySource {
    /// A workspace path.
    Workspace {
        /// The path of this package, relative to the workspace root.
        #[serde(rename = "workspace-path", with = "path_forward_slashes")]
        workspace_path: Utf8PathBuf,
    },

    /// A non-workspace path.
    ///
    /// The path is expected to be relative to the workspace root.
    Path {
        /// The path of this package, relative to the workspace root.
        #[serde(with = "path_forward_slashes")]
        path: Utf8PathBuf,
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
    pub fn workspace(workspace_path: impl Into<Utf8PathBuf>) -> Self {
        SummarySource::Workspace {
            workspace_path: workspace_path.into(),
        }
    }

    /// Creates a new `SummarySource` representing a non-workspace path source.
    pub fn path(path: impl Into<Utf8PathBuf>) -> Self {
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
            // Don't differentiate here between workspace and non-workspace paths because
            // PackageStatus provides that info.
            SummarySource::Workspace { workspace_path } => {
                let path_out = path_forward_slashes::ser_replace_slashes(workspace_path);
                write!(f, "path '{}'", path_out)
            }
            SummarySource::Path { path } => {
                let path_out = path_forward_slashes::ser_replace_slashes(path);
                write!(f, "path '{}'", path_out)
            }
            SummarySource::CratesIo => write!(f, "crates.io"),
            SummarySource::External { source } => write!(f, "external '{}'", source),
        }
    }
}

/// Information about a package in a summary that isn't part of the unique identifier.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct PackageInfo {
    /// Where this package lies in the dependency graph.
    pub status: PackageStatus,

    /// The features built for this package.
    pub features: BTreeSet<String>,
}

/// The status of a package in a summary, such as whether it is part of the initial build set.
///
/// The ordering here determines what order packages will be written out in the summary.
#[derive(Copy, Clone, Debug, Deserialize, Eq, Hash, Ord, Serialize, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum PackageStatus {
    /// This package is part of the requested build set.
    Initial,

    /// This is a workspace package that isn't part of the requested build set.
    Workspace,

    /// This package is a direct non-workspace dependency.
    ///
    /// A `Direct` package may also be transitively included.
    Direct,

    /// This package is a transitive non-workspace dependency.
    Transitive,
}

impl fmt::Display for PackageStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            PackageStatus::Initial => "initial",
            PackageStatus::Workspace => "workspace",
            PackageStatus::Direct => "direct third-party",
            PackageStatus::Transitive => "transitive third-party",
        };
        write!(f, "{}", s)
    }
}

/// Serialization and deserialization for `PackageMap` instances.
mod package_map_impl {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(package_map: &PackageMap, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Make a list of `PackageSerialize` instances and sort by:
        // * status (to ensure initials come first)
        // * summary ID
        let mut package_list: Vec<_> = package_map
            .iter()
            .map(|(summary_id, info)| PackageSerialize { summary_id, info })
            .collect();
        package_list.sort_unstable_by_key(|package| (&package.info.status, package.summary_id));
        package_list.serialize(serializer)
    }

    /// TOML representation of a package in a build summary, for serialization.
    #[derive(Serialize)]
    struct PackageSerialize<'a> {
        #[serde(flatten)]
        summary_id: &'a SummaryId,
        #[serde(flatten)]
        info: &'a PackageInfo,
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<PackageMap, D::Error>
    where
        D: Deserializer<'de>,
    {
        let packages = Vec::<PackageDeserialize>::deserialize(deserializer)?;
        let mut package_map: PackageMap = BTreeMap::new();

        for package in packages {
            package_map.insert(package.summary_id, package.info);
        }
        Ok(package_map)
    }

    /// TOML representation of a package in a build summary, for deserialization.
    #[derive(Deserialize)]
    struct PackageDeserialize {
        #[serde(flatten)]
        summary_id: SummaryId,
        #[serde(flatten)]
        info: PackageInfo,
    }
}

/// Serialization and deserialization for paths that converts to and from forward slashes on
/// Windows.
pub mod path_forward_slashes {
    use super::*;
    use camino::Utf8Path;
    use cfg_if::cfg_if;
    use serde::{Deserializer, Serializer};

    /// Serializes a path using forward slashes.
    pub fn serialize<S>(path: &Utf8PathBuf, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let path_out = ser_replace_slashes(path);
        path_out.serialize(serializer)
    }

    /// Deserializes a path, converting forward slashes to backslashes.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Utf8PathBuf, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(de_replace_slashes(Utf8PathBuf::deserialize(deserializer)?))
    }

    // TODO: consider putting this logic in an actual newtype wrapper :/
    /// Replaces backslashes with forward slashes on Windows.
    pub fn ser_replace_slashes(path: &Utf8Path) -> impl fmt::Display + Serialize + '_ {
        // (Note: serde doesn't support non-Unicode paths anyway.)
        cfg_if! {
            if #[cfg(windows)] {
                path.as_str().replace("\\", "/")
            } else {
                path.as_str()
            }
        }
    }

    /// Replaces forward slashes with backslashes on Windows.
    pub fn de_replace_slashes(path: Utf8PathBuf) -> Utf8PathBuf {
        cfg_if! {
            if #[cfg(windows)] {
                let s = path.into_string();
                // Convert forward slashes to backslashes on Windows.
                let s = s.replace("/", "\\");
                s.into()
            } else {
                path
            }
        }
    }
}

/// Serialization and deserialization for the `CratesIo` variant.
mod crates_io_impl {
    use super::*;
    use serde::{de::Error, ser::SerializeMap, Deserializer, Serializer};

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
