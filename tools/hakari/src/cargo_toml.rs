// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use atomicwrites::{AtomicFile, OverwriteBehavior};
use camino::{Utf8Path, Utf8PathBuf};
use diffy::Patch;
use std::{error, fmt, io};

/// Support for maintaining `Cargo.toml` files that unify features in a workspace.
///
/// This struct maintains a context around a `Cargo.toml` file. It provides facilities for diffing
/// the contents of the file, and for writing out new contents.
///
/// # Structure of the Cargo.toml file
///
/// The `Cargo.toml` file is treated as partially generated. It is expected to have a section
/// marked off as, for example:
///
/// ```toml
/// [package]
/// ...
///
/// ### BEGIN HAKARI SECTION
/// [dependencies]
/// ...
///
/// [build-dependencies]
/// ...
///
/// [dev-dependencies]
/// ...
/// ### END HAKARI SECTION
/// ```
///
/// The part of the `Cargo.toml` file between the `BEGIN HAKARI SECTION` and `END HAKARI SECTION`
/// lines is managed by this struct, and changes to it may not be preserved. The part of the file
/// outside this section can be edited and its contents will be preserved.
///
/// # Setting up a new package
///
/// For Hakari to manage a package, a bit of initial prep work must be done:
///
/// 1. Add a new library package in a desired location within your workspace, for example:
///   `cargo new --lib hakari-package`.
/// 2. Copy and paste the following lines of code to the end of the package's `Cargo.toml` file. Be
///    sure to put in a trailing newline.
///
///     ```toml
///     ### BEGIN HAKARI SECTION
///
///     ### END HAKARI SECTION
///
///     ```
///
/// 3. Add an empty `build.rs` file (the exact contents don't matter, but the presence of this file
///    makes build dependencies work properly).
///
///     ```
///     fn main() {}
///     ```
#[derive(Clone, Debug)]
pub struct HakariCargoToml {
    toml_path: Utf8PathBuf,
    contents: String,
    // Start and end offsets for the section to replace.
    start_offset: usize,
    end_offset: usize,
}

impl HakariCargoToml {
    /// The string `"\n### BEGIN HAKARI SECTION\n"`. This string marks the beginning of the
    /// generated section.
    pub const BEGIN_SECTION: &'static str = "\n### BEGIN HAKARI SECTION\n";

    /// The string `"\n### END HAKARI SECTION\n"`. This string marks the end of the generated
    /// section.
    pub const END_SECTION: &'static str = "\n### END HAKARI SECTION\n";

    /// Creates a new instance of `HakariCargoToml` with the `Cargo.toml` located at the given path.
    /// Reads the contents of the file off of disk.
    ///
    /// If the path is relative, it is evaluated with respect to the current directory.
    ///
    /// Returns an error if the file couldn't be read (other than if the file wasn't found, which
    /// is a case handled by this struct).
    pub fn new(toml_path: impl Into<Utf8PathBuf>) -> Result<Self, CargoTomlError> {
        let toml_path = toml_path.into();

        let contents = match std::fs::read_to_string(&toml_path) {
            Ok(contents) => contents,
            Err(error) => return Err(CargoTomlError::Io { toml_path, error }),
        };

        Self::new_in_memory(toml_path, contents)
    }

    /// Creates a new instance of `HakariCargoToml` at the given workspace root and crate
    /// directory. Reads the contents of the file off of disk.
    ///
    /// This is a convenience method around appending `crate_dir` and `Cargo.toml` to
    /// `workspace_root`.
    ///
    /// If the path is relative, it is evaluated with respect to the current directory.
    pub fn new_relative(
        workspace_root: impl Into<Utf8PathBuf>,
        crate_dir: impl AsRef<Utf8Path>,
    ) -> Result<Self, CargoTomlError> {
        let mut toml_path = workspace_root.into();
        toml_path.push(crate_dir);
        toml_path.push("Cargo.toml");

        Self::new(toml_path)
    }

    /// Creates a new instance of `HakariCargoToml` with the given path with the given contents as
    /// read from disk.
    ///
    /// This may be useful for test scenarios.
    pub fn new_in_memory(
        toml_path: impl Into<Utf8PathBuf>,
        contents: String,
    ) -> Result<Self, CargoTomlError> {
        let toml_path = toml_path.into();

        // Look for the start and end offsets.
        let start_offset = match contents.find(Self::BEGIN_SECTION) {
            Some(offset) => {
                // Add the length of BEGIN_SECTION so that anything after that is replaced.
                offset + Self::BEGIN_SECTION.len()
            }
            None => return Err(CargoTomlError::GeneratedSectionNotFound { toml_path }),
        };

        // Start searching from 1 before the end of the BEGIN text so that we find the END text
        // even if there's nothing in between.
        let end_offset = match contents[(start_offset - 1)..].find(Self::END_SECTION) {
            Some(offset) => start_offset + offset,
            None => return Err(CargoTomlError::GeneratedSectionNotFound { toml_path }),
        };

        Ok(Self {
            toml_path,
            contents,
            start_offset,
            end_offset,
        })
    }

    /// Returns the toml path provided at construction time.
    pub fn toml_path(&self) -> &Utf8Path {
        &self.toml_path
    }

    /// Returns the contents of the file on disk as read at construction time.
    pub fn contents(&self) -> &str {
        &self.contents
    }

    /// Returns the start and end offsets of the part of the file treated as generated.
    pub fn generated_offsets(&self) -> (usize, usize) {
        (self.start_offset, self.end_offset)
    }

    /// Returns the part of the file that is treated as generated.
    ///
    /// This part of the file will be replaced on write.
    pub fn generated_contents(&self) -> &str {
        &self.contents[self.start_offset..self.end_offset]
    }

    /// Returns true if the contents on disk are different from the provided TOML output.
    pub fn is_changed(&self, toml: &str) -> bool {
        self.generated_contents() != toml
    }

    /// Computes the diff between the contents on disk and the provided TOML output.
    ///
    /// This returns a `diffy::Patch`, which can be formatted through methods provided by `diffy`.
    /// `diffy` is re-exported at the top level of this crate.
    ///
    /// # Examples
    ///
    /// TODO
    pub fn diff_toml<'a>(&'a self, toml: &'a str) -> Patch<'a, str> {
        diffy::create_patch(self.generated_contents(), toml)
    }

    /// Writes out the provided TOML to the generated section of the file. The rest of the file is
    /// left unmodified.
    ///
    /// `self` is consumed because the contents of the file are now assumed to be invalid.
    ///
    /// Returns true if the contents were different and the file was written out, false if the
    /// contents were the same and the file was *not* written out, and an error if there was an
    /// issue while writing the file out.
    pub fn write_to_file(self, toml: &str) -> Result<bool, CargoTomlError> {
        if !self.is_changed(toml) {
            // Don't write out the file if it hasn't changed to avoid bumping mtimes.
            return Ok(false);
        }

        let try_block = || {
            let atomic_file = AtomicFile::new(&self.toml_path, OverwriteBehavior::AllowOverwrite);
            atomic_file.write(|f| self.write(toml, f))
        };

        match (try_block)() {
            Ok(()) => Ok(true),
            Err(atomicwrites::Error::Internal(error)) | Err(atomicwrites::Error::User(error)) => {
                Err(CargoTomlError::Io {
                    toml_path: self.toml_path,
                    error,
                })
            }
        }
    }

    /// Writes out the full contents, including the provided TOML, to the given writer.
    pub fn write(&self, toml: &str, mut out: impl io::Write) -> io::Result<()> {
        write!(out, "{}", &self.contents[..self.start_offset])?;
        write!(out, "{}", toml)?;
        write!(out, "{}", &self.contents[self.end_offset..])
    }

    /// Writes out the full contents, including the provided TOML, to the given `fmt::Write`
    /// instance.
    ///
    /// `std::io::Write` expects bytes to be written to it, so using it with a `&mut String` is
    /// inconvenient. This alternative is more convenient, and also works for `fmt::Formatter`
    /// instances.
    pub fn write_to_fmt(&self, toml: &str, mut out: impl fmt::Write) -> fmt::Result {
        // No alternative to copy-pasting :(
        write!(out, "{}", &self.contents[..self.start_offset])?;
        write!(out, "{}", toml)?;
        write!(out, "{}", &self.contents[self.end_offset..])
    }
}

/// An error that can occur while reading or writing a `Cargo.toml` file.
#[derive(Debug)]
#[non_exhaustive]
pub enum CargoTomlError {
    /// The contents of the `Cargo.toml` file could not be read or written.
    Io {
        /// The path that was attempted to be read.
        toml_path: Utf8PathBuf,

        /// The error that occurred.
        error: io::Error,
    },

    /// The `Cargo.toml` was successfully read but `### BEGIN HAKARI SECTION` and
    /// `### END HAKARI SECTION` couldn't be found.
    GeneratedSectionNotFound {
        /// The path that was read.
        toml_path: Utf8PathBuf,
    },
}

impl fmt::Display for CargoTomlError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CargoTomlError::Io { toml_path, .. } => {
                write!(f, "error while reading path '{}'", toml_path)
            }
            CargoTomlError::GeneratedSectionNotFound { toml_path, .. } => {
                write!(
                    f,
                    "in '{}', unable to find\n\
                ### BEGIN HAKARI SECTION\n\
                ...\n\
                ### END HAKARI SECTION",
                    toml_path
                )
            }
        }
    }
}

impl error::Error for CargoTomlError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            CargoTomlError::Io { error, .. } => Some(error),
            CargoTomlError::GeneratedSectionNotFound { .. } => None,
        }
    }
}
