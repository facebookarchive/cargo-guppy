// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Facilities for writing out TOML data from a Hakari map.

use crate::hakari::{HakariBuilder, OutputMap};
#[cfg(feature = "cli-support")]
use crate::summaries::HakariBuilderSummary;
use camino::Utf8PathBuf;
use cfg_if::cfg_if;
use guppy::{
    errors::TargetSpecError,
    graph::{cargo::BuildPlatform, ExternalSource, GitReq, PackageMetadata, PackageSource},
    PackageId, Version,
};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    error, fmt,
    fmt::Write,
    hash::{Hash, Hasher},
};
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
    cfg_if::cfg_if! {
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

    for (key, vals) in output_map {
        let target_str = match key.platform_idx {
            Some(idx) => format!("target.{}.", builder.platforms[idx].triple_str()),
            None => "".to_owned(),
        };
        let dep_str = match key.build_platform {
            BuildPlatform::Target => "dependencies",
            BuildPlatform::Host => "build-dependencies",
        };

        writeln!(out, "[{}{}]", target_str, dep_str)?;

        for (dep, all_features) in vals.values() {
            let mut all_kv: Vec<Cow<str>> = Vec::with_capacity(4);

            // We'd ideally use serde + toml but it doesn't support inline tables. Ugh.
            let name: Cow<str> = if packages_by_name[dep.name()].len() > 1 {
                all_kv.push(format!("package = \"{}\"", dep.name()).into());
                make_hashed_name(dep).into()
            } else {
                dep.name().into()
            };

            let source = dep.source();
            let source_kv = if source.is_crates_io() {
                format!(
                    "version = \"{}\"",
                    VersionDisplay::new(dep.version(), options.exact_versions)
                )
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
                            let rel_path = pathdiff::diff_paths(path, hakari_path)
                                .expect("both hakari_path and path are relative");
                            Utf8PathBuf::from_path_buf(rel_path)
                                .expect("both path and hakari_path are UTF-8 so this is as well")
                        };

                        let path_str = path_out.as_str();
                        cfg_if! {
                            if #[cfg(windows)] {
                                // TODO: is replacing \\ with / totally safe on Windows? Might run
                                // into issues with UNC paths.
                                let path_str = path_str.replace("\\", "/");
                                format!("path = \"{}\"", path_str)
                            } else {
                                format!("path = \"{}\"", path_str)
                            }
                        }
                    }
                    PackageSource::External(s) => match source.parse_external() {
                        Some(ExternalSource::Git {
                            repository, req, ..
                        }) => {
                            let mut out = String::new();
                            write!(out, "git = \"{}\"", repository)?;
                            match req {
                                GitReq::Branch(branch) => write!(out, ", branch = \"{}\"", branch)?,
                                GitReq::Tag(tag) => write!(out, ", tag = \"{}\"", tag)?,
                                GitReq::Rev(rev) => write!(out, ", rev = \"{}\"", rev)?,
                                GitReq::Default => {}
                                _ => {
                                    return Err(TomlOutError::UnrecognizedExternal {
                                        package_id: dep.id().clone(),
                                        source: s.to_string(),
                                    });
                                }
                            };
                            out
                        }
                        Some(ExternalSource::Registry(registry_url)) => {
                            let registry_name = builder
                                .registries
                                .get_by_right(registry_url)
                                .ok_or_else(|| TomlOutError::UnrecognizedRegistry {
                                    package_id: dep.id().clone(),
                                    registry_url: registry_url.to_owned(),
                                })?;
                            format!(
                                "version = \"{}\", registry = \"{}\"",
                                VersionDisplay::new(dep.version(), options.exact_versions),
                                registry_name
                            )
                        }
                        _ => {
                            return Err(TomlOutError::UnrecognizedExternal {
                                package_id: dep.id().clone(),
                                source: s.to_string(),
                            })
                        }
                    },
                }
            };

            all_kv.push(source_kv.into());

            if !all_features.contains("default") {
                all_kv.push("default-features = false".into());
            }

            let features_to_write: Vec<_> = all_features
                .iter()
                .filter_map(|&feature| {
                    if feature == "default" {
                        None
                    } else {
                        Some(format!("\"{}\"", feature))
                    }
                })
                .collect();
            if !features_to_write.is_empty() {
                all_kv.push(format!("features = [{}]", features_to_write.join(", ")).into());
            };

            writeln!(out, "{} = {{ {} }}", name, all_kv.join(", "))?;
        }

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

// TODO: filed https://github.com/steveklabnik/semver/issues/226 about this upstream
/// A formatting wrapper that may print out a minimum version that would match the provided version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct VersionDisplay<'a> {
    version: &'a Version,
    exact_versions: bool,
}

impl<'a> VersionDisplay<'a> {
    fn new(version: &'a Version, exact_versions: bool) -> Self {
        Self {
            version,
            exact_versions,
        }
    }
}

impl<'a> fmt::Display for VersionDisplay<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.exact_versions || !self.version.pre.is_empty() {
            // Preserve the version exactly.
            write!(f, "{}", self.version)
        } else if self.version.major >= 1 {
            write!(f, "{}", self.version.major)
        } else if self.version.minor >= 1 {
            write!(f, "{}.{}", self.version.major, self.version.minor)
        } else {
            write!(
                f,
                "{}.{}.{}",
                self.version.major, self.version.minor, self.version.patch
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixtures::json::*;
    use guppy::{graph::DependencyDirection, VersionReq};
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
    fn min_version() {
        let versions = vec![
            ("1.4.0", "1"),
            ("2.8.0", "2"),
            ("0.4.2", "0.4"),
            ("0.0.7", "0.0.7"),
            ("1.4.0-b1", "1.4.0-b1"),
            ("4.2.3+g456", "4"),
        ];

        for (version_str, min) in versions {
            let version = Version::parse(version_str).expect("valid version");
            let version_req = VersionReq::parse(min).expect("valid version req");
            assert!(
                version_req.matches(&version),
                "version req {} should match version {}",
                min,
                version
            );
            assert_eq!(&format!("{}", VersionDisplay::new(&version, false)), min);
            assert_eq!(
                &format!("{}", VersionDisplay::new(&version, true)),
                version_str
            );
        }
    }

    #[test]
    fn min_versions_match() {
        for (&name, fixture) in JsonFixture::all_fixtures() {
            let graph = fixture.graph();
            for package in graph.resolve_all().packages(DependencyDirection::Forward) {
                let version = package.version();
                let min_version = format!("{}", VersionDisplay::new(version, false));
                let version_req = VersionReq::parse(&min_version).expect("valid version req");

                assert!(
                    version_req.matches(version),
                    "for fixture '{}', for package '{}', min version req {} should match version {}",
                    name,
                    package.id(),
                    min_version,
                    version,
                );
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
