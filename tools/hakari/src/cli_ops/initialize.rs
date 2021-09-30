// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::cli_ops::workspace_ops::{WorkspaceOp, WorkspaceOps};
use camino::{Utf8Path, Utf8PathBuf};
use guppy::graph::PackageGraph;
use include_dir::{include_dir, Dir, DirEntry};
use std::{borrow::Cow, convert::TryInto, error, fmt, io};

const CRATE_TEMPLATE_DIR: Dir = include_dir!("templates/crate");
const CONFIG_TEMPLATE: &str = include_str!("../../templates/hakari.toml-in");

/// Manages initialization of a workspace-hack package.
#[derive(Clone, Debug)]
pub struct HakariInit<'g, 'a> {
    package_graph: &'g PackageGraph,
    package_name: &'a str,
    crate_path: &'a Utf8Path,
    config: Option<(&'a Utf8Path, &'a str)>,
    cargo_toml_comment: &'a str,
}

impl<'g, 'a> HakariInit<'g, 'a> {
    /// Creates a new `HakariInit` with the given options. Writes out a stub config to the path if
    /// specified.
    ///
    /// `crate_path` and `config_path` are relative to the root of the workspace.
    pub fn new(
        package_graph: &'g PackageGraph,
        package_name: &'a str,
        crate_path: &'a Utf8Path,
    ) -> Result<Self, InitError> {
        let workspace = package_graph.workspace();
        let workspace_root = workspace.root();

        // The package name can't already be present in the package graph.
        if let Ok(existing) = workspace.member_by_name(package_name) {
            return Err(InitError::PackageNameExists {
                package_name: package_name.to_owned(),
                workspace_path: existing
                    .source()
                    .workspace_path()
                    .expect("package returned by workspace")
                    .to_owned(),
            });
        }

        let abs_path = workspace_root.join(crate_path);
        if !abs_path.starts_with(workspace.root()) {
            return Err(InitError::WorkspacePathNotInRoot {
                abs_path,
                workspace_root: workspace.root().to_owned(),
            });
        }

        // The workspace path can't already exist (don't follow symlinks for this because even a
        // broken symlink is an error).
        match std::fs::symlink_metadata(&abs_path) {
            Ok(_) => {
                // The path exists.
                return Err(InitError::WorkspacePathExists { abs_path });
            }
            Err(err) => match err.kind() {
                io::ErrorKind::NotFound => {}
                _ => {
                    return Err(InitError::Io {
                        path: abs_path,
                        error: err,
                    });
                }
            },
        }

        // TODO: check package name validity.

        Ok(Self {
            package_graph,
            package_name,
            crate_path,
            config: None,
            cargo_toml_comment: "",
        })
    }

    /// Specifies a path, relative to the workspace root, where a stub configuration file should be
    /// written out. Also accepts a comment (in TOML format) to put at the top of the file.
    ///
    /// If this method is not called, no configuration path will be written out.
    pub fn set_config(
        &mut self,
        path: &'a Utf8Path,
        comment: &'a str,
    ) -> Result<&mut Self, InitError> {
        // The config path can't be present already.
        let abs_path = self.package_graph.workspace().root().join(path);
        if abs_path.exists() {
            return Err(InitError::ConfigPathExists { abs_path });
        }

        self.config = Some((path, comment));
        Ok(self)
    }

    /// Specifies a comment, in TOML format, to add to the top of the workspace-hack package's
    /// `Cargo.toml`.
    pub fn set_cargo_toml_comment(&mut self, comment: &'a str) -> &mut Self {
        self.cargo_toml_comment = comment;
        self
    }

    /// Returns the workspace operations corresponding to this initialization.
    pub fn make_ops(&self) -> WorkspaceOps<'g, 'a> {
        WorkspaceOps::new(
            self.package_graph,
            std::iter::once(self.make_new_crate_op()),
        )
    }

    // ---
    // Helper methods
    // ---

    fn make_new_crate_op(&self) -> WorkspaceOp<'g, 'a> {
        let files = CRATE_TEMPLATE_DIR
            .find("**/*")
            .expect("pattern **/* is valid")
            .flat_map(|entry| {
                match entry {
                    DirEntry::File(file) => {
                        let path: &Utf8Path = file
                            .path()
                            .try_into()
                            .expect("embedded path is valid UTF-8");
                        // .toml-in files need a bit of processing.
                        if path.extension() == Some("toml-in") {
                            let contents = file
                                .contents_utf8()
                                .expect("embedded .toml-in is valid UTF-8");
                            let contents = contents.replace("%PACKAGE_NAME%", self.package_name);
                            let contents =
                                contents.replace("%CARGO_TOML_COMMENT%\n", self.cargo_toml_comment);
                            Some((
                                Cow::Owned(path.with_extension("toml")),
                                Cow::Owned(contents.into_bytes()),
                            ))
                        } else {
                            Some((Cow::Borrowed(path), Cow::Borrowed(file.contents())))
                        }
                    }
                    DirEntry::Dir(_) => None,
                }
            })
            .collect();

        let root_files = self
            .config
            .into_iter()
            .map(|(path, comment)| {
                let contents = CONFIG_TEMPLATE.replace("%PACKAGE_NAME%", self.package_name);
                let contents = contents.replace("%CONFIG_COMMENT%\n", comment);
                (Cow::Borrowed(path), Cow::Owned(contents.into_bytes()))
            })
            .collect();

        WorkspaceOp::NewCrate {
            crate_path: self.crate_path,
            files,
            root_files,
        }
    }
}

/// An error that occurred while attempting to initialize `hakari`.
#[derive(Debug)]
#[non_exhaustive]
pub enum InitError {
    /// The configuration path already exists.
    ConfigPathExists {
        /// The absolute path of the configuration file.
        abs_path: Utf8PathBuf,
    },

    /// The provided package name already exists.
    PackageNameExists {
        /// The package name that exists.
        package_name: String,

        /// The path at which it exists, relative to the root.
        workspace_path: Utf8PathBuf,
    },

    /// The provided path is not within the workspace root.
    WorkspacePathNotInRoot {
        /// The absolute workspace path.
        abs_path: Utf8PathBuf,

        /// The workspace root.
        workspace_root: Utf8PathBuf,
    },

    /// The provided workspace directory is non-empty.
    WorkspacePathExists {
        /// The absolute workspace path.
        abs_path: Utf8PathBuf,
    },

    /// An IO error occurred while working with the given path.
    Io {
        /// The path.
        path: Utf8PathBuf,

        /// The error.
        error: io::Error,
    },
}

impl fmt::Display for InitError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            InitError::ConfigPathExists { abs_path } => {
                write!(f, "config already exists at path {}", abs_path)
            }
            InitError::PackageNameExists {
                package_name,
                workspace_path,
            } => {
                write!(
                    f,
                    "package name {} already exists at path {}",
                    package_name, workspace_path
                )
            }
            InitError::WorkspacePathNotInRoot {
                abs_path,
                workspace_root,
            } => {
                write!(
                    f,
                    "path {} is not within workspace {}",
                    abs_path, workspace_root
                )
            }
            InitError::WorkspacePathExists { abs_path } => {
                write!(f, "workspace path {} already exists", abs_path)
            }
            InitError::Io { path, .. } => {
                write!(f, "IO error while accessing {}", path)
            }
        }
    }
}

impl error::Error for InitError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            InitError::Io { error, .. } => Some(error),
            InitError::ConfigPathExists { .. }
            | InitError::PackageNameExists { .. }
            | InitError::WorkspacePathNotInRoot { .. }
            | InitError::WorkspacePathExists { .. } => None,
        }
    }
}
