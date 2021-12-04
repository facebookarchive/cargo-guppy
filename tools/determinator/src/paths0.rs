// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use camino::Utf8Path;
use std::str::Utf8Error;

/// A store for null-separated paths.
///
/// This manages paths on Unix and Windows platforms, including converting `/` on Windows to `\`.
///
/// # Null-separated paths
///
/// Paths as produced by tools like `git diff --name-only` are typically separated by newline
/// characters (`\n`). However, on Unix platforms filenames can themselves have newlines embedded in
/// them, so source control systems often end up
/// [quoting newlines and other "unusual" characters](https://git-scm.com/docs/git-config#Documentation/git-config.txt-corequotePath).
///
/// A robust, lossless way to retrieve a list of paths is by separating them with null characters.
/// Both Unix and Windows platforms guarantee that a path can never have embedded null characters.
///
/// # Examples
///
/// Most source control systems can provide null-separated paths. These examples are expected to be
/// run from the Cargo workspace root (which is assumed to be the same as the repository root).
///
/// In most cases, you'll want to compare the current working directory against the [*merge base*][mb],
/// or [*nearest/greatest/lowest common ancestor*](https://en.wikipedia.org/wiki/Lowest_common_ancestor),
/// of the current commit with a specified upstream revision, such as `origin/main`. To do so,
/// run:
///
/// * Git: `git diff -z --name-only $(git merge-base <upstream rev> HEAD)`
/// * Mercurial: `hg status --print0 -mard --no-status --rev 'ancestor(<upstream rev>,.)'`
///
/// [mb]: https://stackoverflow.com/questions/1549146/git-find-the-most-recent-common-ancestor-of-two-branches
///
/// ---
///
/// **NOTE:**
/// * The `$()` syntax in Bash and other shells means "run the command and insert its contents here".
/// * Git provides a syntax `<upstream rev>...` which purports to use the merge base,
/// but it ignores uncommitted changes. Executing `git merge-base` as a separate command is the only
/// way to include uncommitted changes.
/// * The `-mard` flag to `hg status` means that untracked files are not included. `git diff` does
///   not have an option to display untracked files. For more discussion, see the documentation for
///   [`add_changed_paths`](crate::Determinator::add_changed_paths).
///
/// ---
///
/// In general, to obtain a list of changed paths between two revisions (omit `<new rev>`
/// if comparing against the working directory):
///
/// * Git: `git diff -z --name-only <old rev> <new rev>`
/// * Mercurial: `hg status --print0 -mard --no-status <old rev> <new rev>`
///
/// To obtain a list of all files in the working directory that are tracked by the source control
/// system:
///
/// * Git: `git ls-files -z`
/// * Mercurial: `hg files --print0`
///
/// Null-separated paths are produced through the `-z` option to Git commands, or the `--print0`
/// option to Mercurial. If you're using a different system, check its help for instructions.
///
/// # Implementations
///
/// `&'a Utf8Paths0` implements `IntoIterator<Item = &'a Utf8Path>`.
#[derive(Clone, Debug, Eq, Ord, PartialOrd, PartialEq)]
pub struct Utf8Paths0 {
    buf: Box<str>,
}

impl Utf8Paths0 {
    /// Creates a new instance of `Utf8Paths0` from a string with embedded nulls.
    ///
    /// The string may, but does not need to, have a trailing null byte.
    pub fn new(buf: impl Into<String>) -> Self {
        Self::strip_trailing_null_byte(buf.into())
    }

    /// Creates a new instance of `Utf8Paths0` from a `Vec<u8>`, performing a UTF-8 validation
    /// check on the buffer.
    ///
    /// The buffer may, but does not need to, have a trailing null byte.
    ///
    /// ## Errors
    ///
    /// If any paths inside the string weren't valid UTF-8, this returns the first path that failed
    /// to parse and the error returned.
    pub fn from_bytes(buf: impl Into<Vec<u8>>) -> Result<Self, (Vec<u8>, Utf8Error)> {
        let buf = buf.into();
        let buf = Self::validate_utf8(buf)?;
        Ok(Self::strip_trailing_null_byte(buf))
    }

    /// Creates a new instance of `Utf8Paths0`, converting `/` to `\` on platforms like Windows.
    ///
    /// Some tools like Git (but not Mercurial) return paths with `/` on Windows, even though the
    /// canonical separator on the platform is `\`. This constructor changes all instances of `/`
    /// to `\`.
    pub fn new_forward_slashes(buf: impl Into<String>) -> Self {
        let mut buf = buf.into();
        // Change all `/` to `\` on Windows.
        if std::path::MAIN_SEPARATOR == '\\' {
            buf = buf.replace('/', "\\");
        }
        Self::strip_trailing_null_byte(buf)
    }

    /// Iterates over the paths in this buffer.
    pub fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Utf8Path> + 'a> {
        self.into_iter()
    }

    // ---
    // Helper methods
    // ---

    fn validate_utf8(buf: Vec<u8>) -> Result<String, (Vec<u8>, Utf8Error)> {
        match String::from_utf8(buf) {
            Ok(s) => Ok(s),
            Err(err) => {
                let buf = err.into_bytes();
                // Look for the path that failed validation.
                buf.split(|b| *b == 0)
                    .try_for_each(|path| match std::str::from_utf8(path) {
                        Ok(_) => Ok(()),
                        Err(utf8_error) => Err((path.to_vec(), utf8_error)),
                    })?;
                unreachable!("full buffer failed utf-8 validation => at least one path failed");
            }
        }
    }

    fn strip_trailing_null_byte(mut buf: String) -> Self {
        if buf.as_bytes().last() == Some(&0) {
            buf.pop();
        }

        Self { buf: buf.into() }
    }
}

impl<'a> IntoIterator for &'a Utf8Paths0 {
    type Item = &'a Utf8Path;
    type IntoIter = Box<dyn Iterator<Item = &'a Utf8Path> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        // An empty string means there are no paths -- this has to be handled as a special case.
        if self.buf.is_empty() {
            return Box::new(std::iter::empty());
        }

        Box::new(self.buf.split('\0').map(Utf8Path::new))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        // Empty string should return no paths.
        paths_eq(*b"", &[]);

        paths_eq(*b"a/b/c", &["a/b/c"]);
        paths_eq(*b"a/b\0a/c", &["a/b", "a/c"]);
        paths_eq(*b"a/b\0a/c\0", &["a/b", "a/c"]);

        // UTF-8
        paths_eq(*b"a/b\xF0\x9F\x98\x81\0c/d", &["a/b😁", "c/d"]);
    }

    // This is really a Windows test but it should work on all platforms.
    #[test]
    fn backslashes() {
        paths_eq(*b"a\\b\\c", &["a\\b\\c"]);
        paths_eq(*b"a\\b\0a\\c", &["a\\b", "a\\c"]);
        paths_eq(*b"a\\b\0a\\c\0", &["a\\b", "a\\c"]);
    }

    #[cfg(windows)]
    #[test]
    fn forward_slashes() {
        paths_eq_fwd(*b"a/b/c", &["a\\b\\c"]);
        paths_eq_fwd(*b"a/b\0a/c", &["a\\b", "a\\c"]);
        paths_eq_fwd(*b"a/b\0a/c\0", &["a\\b", "a\\c"]);

        // Also test mixed forward/backslashes.
        paths_eq_fwd(*b"a/b\0a\\c", &["a\\b", "a\\c"]);
    }

    fn paths_eq(bytes: impl Into<Vec<u8>>, expected: &[&str]) {
        let paths = Utf8Paths0::from_bytes(bytes.into()).expect("null-separated paths are valid");
        let actual: Vec<_> = paths.iter().collect();
        let expected: Vec<_> = expected.iter().map(Utf8Path::new).collect();

        assert_eq!(actual, expected, "paths match");
    }

    #[cfg(windows)]
    fn paths_eq_fwd(bytes: impl Into<Vec<u8>>, expected: &[&str]) {
        let s = String::from_utf8(bytes.into()).expect("valid UTF-8");
        let paths = Utf8Paths0::new_forward_slashes(s);
        let actual: Vec<_> = paths.iter().collect();
        let expected: Vec<_> = expected.iter().map(Utf8Path::new).collect();

        assert_eq!(actual, expected, "paths match");
    }
}
