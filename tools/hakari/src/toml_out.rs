// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Facilities for writing out TOML data from a Hakari map.

use crate::hakari::{HakariBuilder, OutputMap};
use crate::helpers::VersionDisplay;
#[cfg(feature = "cli-support")]
use crate::summaries::HakariBuilderSummary;
use camino::Utf8PathBuf;
use cfg_if::cfg_if;
use guppy::{
    errors::TargetSpecError,
    graph::{cargo::BuildPlatform, ExternalSource, GitReq, PackageMetadata, PackageSource},
    PackageId,
};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    error, fmt,
    hash::{Hash, Hasher},
};
use toml_edit::{Array, Document, InlineTable, Item, Table, Value};
use twox_hash::XxHash64;

/// Options for Hakari TOML output.
#[derive(Clone, Debug)]
pub struct HakariOutputOptions {
    pub(crate) exact_versions: bool,
    pub(crate) absolute_paths: bool,
    #[cfg(feature = "cli-support")]
    pub(crate) builder_summary: bool,
}

impl HakariOutputOptions {
    /// Creates a new instance with default settings.
    ///
    /// The default settings are:
    /// * do not output exact versions
    /// * do not output a summary of builder options
    pub fn new() -> Self {
        Self {
            exact_versions: false,
            absolute_paths: false,
            #[cfg(feature = "cli-support")]
            builder_summary: false,
        }
    }

    /// If set to true, outputs exact versions in package version fields.
    ///
    /// By default, Hakari outputs the loosest possible version requirement that matches the
    /// specified package. This is generally appropriate  if the `Cargo.lock` file isn't checked in,
    /// and there's no automated process keeping the dependencies up-to-date.
    ///
    /// In some cases one may wish to output the exact versions selected instead. For example:
    /// * The `Cargo.lock` file is checked in and all developers have matching lockfiles.
    /// * A tool like [Dependabot](https://dependabot.com/) is configured to update `Cargo.toml`
    ///   files to their latest versions.
    ///
    /// ## Note
    ///
    /// If set to true, and the `Cargo.lock` file isn't checked in, Hakari's output will vary based
    /// on the repository it is run in. Most of the time this isn't desirable.
    pub fn set_exact_versions(&mut self, exact_versions: bool) -> &mut Self {
        self.exact_versions = exact_versions;
        self
    }

    /// If set to true, outputs absolute paths for path dependencies.
    ///
    /// By default, `hakari` outputs relative paths, for example:
    ///
    /// ```toml
    /// path-dependency = { path = "../../path-dependency" }
    /// ```
    ///
    /// If set to true, `hakari` will output absolute paths, for example:
    ///
    /// ```toml
    /// path-dependency = { path = "/path/to/path-dependency" }
    /// ```
    ///
    /// In most situations, relative paths lead to better results. Use with care.
    ///
    /// ## Notes
    ///
    /// If set to false, a Hakari package must be specified in [`HakariBuilder`](HakariBuilder). If
    /// it isn't and Hakari needs to output a relative path,
    /// [`TomlOutError::PathWithoutHakari`](TomlOutError::PathWithoutHakari) will be returned.
    pub fn set_absolute_paths(&mut self, absolute_paths: bool) -> &mut Self {
        self.absolute_paths = absolute_paths;
        self
    }

    /// If set to true, outputs a summary of the builder options used to generate the `Hakari`, as
    /// TOML comments.
    ///
    /// The options are output as a header in the Hakari section:
    ///
    /// ```toml
    /// # resolver = "2"
    /// # platforms = [...]
    /// ...
    /// ```
    ///
    /// Requires the `cli-support` feature to be enabled.
    #[cfg(feature = "cli-support")]
    pub fn set_builder_summary(&mut self, builder_summary: bool) -> &mut Self {
        self.builder_summary = builder_summary;
        self
    }
}

impl Default for HakariOutputOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// An error that occurred while writing out TOML.
#[derive(Debug)]
#[non_exhaustive]
pub enum TomlOutError {
    /// An error occurred while serializing platform information.
    Platform(TargetSpecError),

    /// An error occurred while serializing TOML.
    ///
    /// This option is only present if the `cli-support` feature is enabled.
    #[cfg(feature = "cli-support")]
    Toml {
        /// A context string for the error.
        context: Cow<'static, str>,

        /// The underlying error.
        err: toml::ser::Error,
    },

    /// An error occurred while writing to a `fmt::Write` instance.
    FmtWrite(fmt::Error),

    /// Attempted to output a path dependency, but a Hakari package wasn't provided to the builder.
    ///
    /// If any path dependencies need to be unified, the location of the Hakari package must be
    /// specified so that a relative path can be displayed.
    PathWithoutHakari {
        /// The package ID that Hakari tried to write out a dependency line for.
        package_id: PackageId,

        /// The relative path to the package from the root of the workspace.
        rel_path: Utf8PathBuf,
    },

    /// An external source wasn't recognized by guppy.
    UnrecognizedExternal {
        /// The package ID that Hakari tried to write out a dependency line for.
        package_id: PackageId,

        /// The source string that wasn't recognized.
        source: String,
    },

    /// An external registry was found and wasn't passed into [`HakariOutputOptions`].
    UnrecognizedRegistry {
        /// The package ID that Hakari tried to write out a dependency line for.
        package_id: PackageId,

        /// The registry URL that wasn't recognized.
        registry_url: String,
    },
}

impl From<TargetSpecError> for TomlOutError {
    fn from(err: TargetSpecError) -> Self {
        TomlOutError::Platform(err)
    }
}

impl From<fmt::Error> for TomlOutError {
    fn from(err: fmt::Error) -> Self {
        TomlOutError::FmtWrite(err)
    }
}

impl fmt::Display for TomlOutError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TomlOutError::Platform(_) => write!(f, "while serializing platform information"),
            #[cfg(feature = "cli-support")]
            TomlOutError::Toml { context, .. } => write!(f, "while serializing TOML: {}", context),
            TomlOutError::FmtWrite(_) => write!(f, "while writing to fmt::Write"),
            TomlOutError::PathWithoutHakari {
                package_id,
                rel_path,
            } => write!(
                f,
                "for path dependency '{}', no Hakari package was specified (relative path {})",
                package_id, rel_path,
            ),
            TomlOutError::UnrecognizedExternal { package_id, source } => write!(
                f,
                "for third-party dependency '{}', unrecognized external source {}",
                package_id, source,
            ),
            TomlOutError::UnrecognizedRegistry {
                package_id,
                registry_url,
            } => {
                write!(
                    f,
                    "for third-party dependency '{}', unrecognized registry at URL {}",
                    package_id, registry_url,
                )
            }
        }
    }
}

impl error::Error for TomlOutError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            TomlOutError::Platform(err) => Some(err),
            #[cfg(feature = "cli-support")]
            TomlOutError::Toml { err, .. } => Some(err),
            TomlOutError::FmtWrite(err) => Some(err),
            TomlOutError::PathWithoutHakari { .. }
            | TomlOutError::UnrecognizedExternal { .. }
            | TomlOutError::UnrecognizedRegistry { .. } => None,
        }
    }
}

pub(crate) fn write_toml(
    builder: &HakariBuilder<'_>,
    output_map: &OutputMap<'_>,
    options: &HakariOutputOptions,
    mut out: impl fmt::Write,
) -> Result<(), TomlOutError> {
    cfg_if! {
        if #[cfg(feature = "cli-support")] {
            if options.builder_summary {
                let summary = HakariBuilderSummary::new(builder)?;
                summary.write_comment(&mut out)?;
                writeln!(out)?;
            }
        }
    }

    let mut packages_by_name: HashMap<&str, HashSet<_>> = HashMap::new();
    for vals in output_map.values() {
        for (&package_id, (package, _)) in vals {
            packages_by_name
                .entry(package.name())
                .or_default()
                .insert(package_id);
        }
    }

    let hakari_path = builder.hakari_package().map(|package| {
        package
            .source()
            .workspace_path()
            .expect("hakari package is in workspace")
    });

    let mut document = Document::new();

    // Remove the leading newline from the first visual table to match what older versions of
    // hakari did.
    let mut first_element = true;

    for (key, vals) in output_map {
        let dep_table_parent = match key.platform_idx {
            Some(idx) => {
                let target_table = get_or_insert_table(document.as_table_mut(), "target");
                get_or_insert_table(target_table, builder.platforms[idx].triple_str())
            }
            None => document.as_table_mut(),
        };

        let dep_table = match key.build_platform {
            BuildPlatform::Target => get_or_insert_table(dep_table_parent, "dependencies"),
            BuildPlatform::Host => get_or_insert_table(dep_table_parent, "build-dependencies"),
        };

        if first_element {
            dep_table.decor_mut().set_prefix("");
            first_element = false;
        }

        for (dep, all_features) in vals.values() {
            let mut itable = InlineTable::new();

            let name: Cow<str> = if packages_by_name[dep.name()].len() > 1 {
                itable.insert("package", dep.name().into());
                make_hashed_name(dep).into()
            } else {
                dep.name().into()
            };

            let source = dep.source();
            if source.is_crates_io() {
                itable.insert(
                    "version",
                    format!(
                        "{}",
                        VersionDisplay::new(dep.version(), options.exact_versions)
                    )
                    .into(),
                );
            } else {
                match source {
                    PackageSource::Workspace(path) | PackageSource::Path(path) => {
                        // PackageSource::Workspace shouldn't be possible unless the Hakari map
                        // was fiddled with. Regardless, we can handle it fine.
                        let path_out = if options.absolute_paths {
                            // TODO: canonicalize paths here, removing .. etc? tricky if the path is
                            // missing (as in tests)
                            builder.graph().workspace().root().join(path)
                        } else {
                            let hakari_path =
                                hakari_path.ok_or_else(|| TomlOutError::PathWithoutHakari {
                                    package_id: dep.id().clone(),
                                    rel_path: path.to_path_buf(),
                                })?;
                            pathdiff::diff_utf8_paths(path, hakari_path)
                                .expect("both hakari_path and path are relative")
                        }
                        .into_string();

                        cfg_if! {
                            if #[cfg(windows)] {
                                // TODO: is replacing \\ with / totally safe on Windows? Might run
                                // into issues with UNC paths.
                                let path_out = path_out.replace("\\", "/");
                                itable.insert("path", path_out.into());
                            } else {
                                itable.insert("path", path_out.into());
                            }
                        };
                    }
                    PackageSource::External(s) => match source.parse_external() {
                        Some(ExternalSource::Git {
                            repository, req, ..
                        }) => {
                            itable.insert("git", repository.into());
                            match req {
                                GitReq::Branch(branch) => {
                                    itable.insert("branch", branch.into());
                                }
                                GitReq::Tag(tag) => {
                                    itable.insert("tag", tag.into());
                                }
                                GitReq::Rev(rev) => {
                                    itable.insert("rev", rev.into());
                                }
                                GitReq::Default => {}
                                _ => {
                                    return Err(TomlOutError::UnrecognizedExternal {
                                        package_id: dep.id().clone(),
                                        source: s.to_string(),
                                    });
                                }
                            };
                        }
                        Some(ExternalSource::Registry(registry_url)) => {
                            let registry_name = builder
                                .registries
                                .get_by_right(registry_url)
                                .ok_or_else(|| TomlOutError::UnrecognizedRegistry {
                                    package_id: dep.id().clone(),
                                    registry_url: registry_url.to_owned(),
                                })?;
                            itable.insert(
                                "version",
                                format!(
                                    "{}",
                                    VersionDisplay::new(dep.version(), options.exact_versions)
                                )
                                .into(),
                            );
                            itable.insert("registry", registry_name.into());
                        }
                        _ => {
                            return Err(TomlOutError::UnrecognizedExternal {
                                package_id: dep.id().clone(),
                                source: s.to_string(),
                            });
                        }
                    },
                }
            };

            if !all_features.contains("default") {
                itable.insert("default-features", false.into());
            }

            let feature_array: Array = all_features
                .iter()
                .filter_map(|&feature| (feature != "default").then(|| feature))
                .collect();
            if !feature_array.is_empty() {
                itable.insert("features", feature_array.into());
            }

            itable.fmt();

            dep_table.insert(name.as_ref(), Item::Value(Value::InlineTable(itable)));
        }
    }

    // Match formatting with older versions of hakari: if the document is non-empty, print out a
    // newline at the end.
    write!(out, "{}", document)?;
    if !document.is_empty() {
        writeln!(out)?;
    }

    Ok(())
}

/// Generate a unique, stable package name from the metadata.
fn make_hashed_name(dep: &PackageMetadata<'_>) -> String {
    // Use a fixed seed to ensure stable hashes.
    let mut hasher = XxHash64::default();
    // Use the minimal version so that a bump from e.g. 0.2.5 to 0.2.6 doesn't change the hash.
    let minimal_version = format!("{}", VersionDisplay::new(dep.version(), false));
    minimal_version.hash(&mut hasher);
    dep.source().hash(&mut hasher);
    let hash = hasher.finish();

    format!("{}-{:x}", dep.name(), hash)
}

fn get_or_insert_table<'t>(parent: &'t mut Table, key: &str) -> &'t mut Table {
    let table = parent
        .entry(key)
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .expect("just inserted this table");
    table.set_implicit(true);
    table
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixtures::json::*;
    use guppy::graph::DependencyDirection;
    use std::collections::{btree_map::Entry, BTreeMap};

    #[test]
    fn make_package_name_unique() {
        for (&name, fixture) in JsonFixture::all_fixtures() {
            let mut names_seen: BTreeMap<String, PackageMetadata<'_>> = BTreeMap::new();
            let graph = fixture.graph();
            for package in graph.resolve_all().packages(DependencyDirection::Forward) {
                match names_seen.entry(make_hashed_name(&package)) {
                    Entry::Vacant(entry) => {
                        entry.insert(package);
                    }
                    Entry::Occupied(entry) => {
                        panic!(
                            "for fixture '{}', duplicate generated package name '{}'. packages\n\
                        * {}\n\
                        * {}",
                            name,
                            entry.key(),
                            entry.get().id(),
                            package.id()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn alternate_registries() {
        let fixture = JsonFixture::metadata_alternate_registries();
        let mut builder =
            HakariBuilder::new(fixture.graph(), None).expect("builder initialization succeeded");
        builder.set_output_single_feature(true);
        let hakari = builder.compute();

        // Not plugging in the registry should generate an error.
        let output_options = HakariOutputOptions::new();
        hakari
            .to_toml_string(&output_options)
            .expect_err("no alternate registry specified => error");

        let mut builder =
            HakariBuilder::new(fixture.graph(), None).expect("builder initialization succeeded");
        builder.set_output_single_feature(true);
        builder.add_registries([("alt-registry", METADATA_ALTERNATE_REGISTRY_URL)]);
        let hakari = builder.compute();

        let output = hakari
            .to_toml_string(&output_options)
            .expect("alternate registry specified => success");

        static MATCH_STRINGS: &[&str] = &[
            // Two copies of serde, one from the main registry and one from the alt
            r#"serde-e7e45184a9cd0878 = { package = "serde", version = "1", registry = "alt-registry", default-features = false, "#,
            r#"serde-dff4ba8e3ae991db = { package = "serde", version = "1", default-features = false, "#,
            // serde_derive only in the alt registry
            r#"serde_derive = { version = "1", registry = "alt-registry" }"#,
            // itoa only from the main registry
            r#"itoa = { version = "0.4", default-features = false }"#,
        ];

        for &needle in MATCH_STRINGS {
            assert!(
                output.contains(needle),
                "output did not contain string '{}', actual output follows:\n***\n{}\n",
                needle,
                output
            );
        }
    }
}
