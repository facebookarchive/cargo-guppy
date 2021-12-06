// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{cargo_cli::CargoCli, output::OutputContext};
use camino::{Utf8Path, Utf8PathBuf};
use color_eyre::{
    eyre::{bail, WrapErr},
    Result,
};

/// Read the contents of the first file that matches and is present. Errors out.
pub(crate) fn read_contents(
    root: &Utf8Path,
    rel_paths: impl IntoIterator<Item = impl AsRef<Utf8Path>>,
) -> Result<(Utf8PathBuf, String)> {
    let mut paths_tried_str = String::new();
    for path in rel_paths {
        let abs_path = root.join(path);
        match std::fs::read_to_string(&abs_path) {
            Ok(contents) => return Ok((abs_path, contents)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                // The path wasn't found -- continue to the next one.
                paths_tried_str.push_str("  - ");
                paths_tried_str.push_str(abs_path.as_str());
                paths_tried_str.push('\n');
                continue;
            }
            Err(err) => {
                return Err(err)
                    .wrap_err_with(|| format!("error reading contents at {}", abs_path));
            }
        }
    }

    bail!("none of these paths were found:\n{}", paths_tried_str)
}

/// Regenerate the lockfile after dependency updates.
pub(crate) fn regenerate_lockfile(output: OutputContext) -> Result<()> {
    // This seems to be the cheapest way to update the lockfile.
    // cargo update -p <hakari-package> can sometimes cause unnecessary index updates.
    let cargo_cli = CargoCli::new("tree", output);
    cargo_cli
        .to_expression()
        .stdout_null()
        .run()
        .wrap_err("updating Cargo.lock failed")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryInto;
    use tempfile::TempDir;

    #[test]
    fn test_read_contents() -> Result<()> {
        let dir = TempDir::new()?;
        let root: &Utf8Path = dir.path().try_into().expect("path is UTF-8");
        std::fs::write(dir.path().join("foo"), "foo-contents")?;
        std::fs::write(dir.path().join("bar"), "bar-contents")?;

        assert_eq!(
            read_contents(root, ["foo", "bar"]).unwrap().1,
            "foo-contents"
        );
        assert_eq!(
            read_contents(root, ["bar", "foo"]).unwrap().1,
            "bar-contents"
        );
        assert_eq!(
            read_contents(root, ["missing", "foo"]).unwrap().1,
            "foo-contents"
        );
        println!(
            "{}",
            read_contents(root, ["missing", "missing-2"]).unwrap_err(),
        );

        Ok(())
    }
}
