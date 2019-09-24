// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::Lockfile;
use crate::PackageId;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct DiffOptions;

impl DiffOptions {
    pub fn diff(&self, old_lockfile: &Lockfile, new_lockfile: &Lockfile) -> Diff {
        let mut new = new_lockfile.packages().clone();

        let mut removed = old_lockfile
            .packages()
            .iter()
            .filter_map(|(pkg_id, _pkg)| {
                if new.remove(pkg_id).is_none() {
                    Some(pkg_id.clone())
                } else {
                    None
                }
            })
            .map(|removed_pkg_id| {
                let remaining_packages = new_lockfile
                    .packages()
                    .iter()
                    .filter(|(pkg_id, _)| {
                        (**pkg_id != removed_pkg_id) && (pkg_id.name() == removed_pkg_id.name())
                    })
                    .map(|(pkg_id, _)| pkg_id.clone())
                    .collect::<Vec<_>>();

                if remaining_packages.is_empty() {
                    (removed_pkg_id.clone(), None)
                } else {
                    (removed_pkg_id.clone(), Some(remaining_packages))
                }
            })
            .collect::<HashMap<_, _>>();

        let mut added = new
            .into_iter()
            .map(|(added_pkg_id, _pkg)| {
                let existing_packages = new_lockfile
                    .packages()
                    .iter()
                    .filter(|(pkg_id, _)| {
                        (**pkg_id != added_pkg_id) && (pkg_id.name() == added_pkg_id.name())
                    })
                    .map(|(pkg_id, _)| pkg_id.clone())
                    .collect::<Vec<_>>();

                if existing_packages.is_empty() {
                    (added_pkg_id, None)
                } else {
                    (added_pkg_id, Some(existing_packages))
                }
            })
            .collect::<HashMap<_, _>>();

        let mut updated = removed
            .iter()
            .filter_map(|(removed_pkg_id, _)| {
                if let Some((updated, _)) = added
                    .iter()
                    .find(|added_pkg| removed_pkg_id.name() == added_pkg.0.name())
                {
                    Some((removed_pkg_id.clone(), updated.clone()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        updated.sort_by(|a, b| a.0.name().cmp(b.0.name()));

        // Remove entries from Added and Removed
        for (removed_pkg_id, added_pkg_id) in &updated {
            removed.remove(removed_pkg_id);
            added.remove(added_pkg_id);
        }

        let mut removed = removed.into_iter().collect::<Vec<_>>();
        removed.sort_by(|(a, _), (b, _)| a.name().cmp(b.name()));
        let mut added = added.into_iter().collect::<Vec<_>>();
        added.sort_by(|(a, _), (b, _)| a.name().cmp(b.name()));

        Diff {
            updated,
            removed,
            added,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Diff {
    updated: Vec<(PackageId, PackageId)>,
    removed: Vec<(PackageId, Option<Vec<PackageId>>)>,
    added: Vec<(PackageId, Option<Vec<PackageId>>)>,
}

impl ::std::fmt::Display for Diff {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        fn write_dups(
            f: &mut ::std::fmt::Formatter<'_>,
            dups: &Option<Vec<PackageId>>,
        ) -> ::std::fmt::Result {
            if let Some(dups) = dups {
                write!(f, " ({}", dups[0].version())?;
                for p in &dups[1..] {
                    write!(f, ", {}", p.version())?;
                }
                write!(f, ")")?;
            }

            Ok(())
        }

        if !self.added.is_empty() {
            writeln!(f, "Added Packages (Duplicate versions in '()'):")?;
            for (added, dups) in &self.added {
                write!(f, "\t{} {}", added.name(), added.version(),)?;

                write_dups(f, dups)?;
                writeln!(f)?;
            }
            writeln!(f)?;
        }

        if !self.removed.is_empty() {
            writeln!(f, "Removed Packages (Remaining versions in '()'):")?;
            for (removed, dups) in &self.removed {
                write!(f, "\t{} {}", removed.name(), removed.version(),)?;

                write_dups(f, dups)?;
                writeln!(f)?;
            }
            writeln!(f)?;
        }

        if !self.updated.is_empty() {
            writeln!(f, "Updated Packages:")?;
            for (removed, added) in &self.updated {
                writeln!(
                    f,
                    "\t{}: {} -> {}",
                    removed.name(),
                    removed.version(),
                    added.version(),
                )?;
            }
            writeln!(f)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{diff::DiffOptions, lockfile::Lockfile};

    #[test]
    fn simple_diff() {
        let old = r#"
            [[package]]
            name = "cargo-guppy"
            version = "0.1.0"
            dependencies = [
             "serde 1.0.99 (registry+https://github.com/rust-lang/crates.io-index)",
             "toml 0.5.3 (registry+https://github.com/rust-lang/crates.io-index)",
            ]

            [[package]]
            name = "serde"
            version = "1.0.99"
            source = "registry+https://github.com/rust-lang/crates.io-index"

            [[package]]
            name = "toml"
            version = "0.5.3"
            source = "registry+https://github.com/rust-lang/crates.io-index"
            dependencies = [
             "serde 1.0.99 (registry+https://github.com/rust-lang/crates.io-index)",
            ]

            [metadata]
            "checksum serde 1.0.99 (registry+https://github.com/rust-lang/crates.io-index)" = "fec2851eb56d010dc9a21b89ca53ee75e6528bab60c11e89d38390904982da9f"
            "checksum toml 0.5.3 (registry+https://github.com/rust-lang/crates.io-index)" = "c7aabe75941d914b72bf3e5d3932ed92ce0664d49d8432305a8b547c37227724"
        "#;

        let new = r#"
            [[package]]
            name = "cargo-guppy"
            version = "0.1.0"
            dependencies = [
             "serde 1.0.99 (registry+https://github.com/rust-lang/crates.io-index)",
             "toml 0.5.3 (registry+https://github.com/rust-lang/crates.io-index)",
            ]

            [[package]]
            name = "proc-macro2"
            version = "1.0.2"
            source = "registry+https://github.com/rust-lang/crates.io-index"
            dependencies = [
             "unicode-xid 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)",
            ]

            [[package]]
            name = "quote"
            version = "1.0.2"
            source = "registry+https://github.com/rust-lang/crates.io-index"
            dependencies = [
             "proc-macro2 1.0.2 (registry+https://github.com/rust-lang/crates.io-index)",
            ]

            [[package]]
            name = "serde"
            version = "1.0.99"
            source = "registry+https://github.com/rust-lang/crates.io-index"
            dependencies = [
             "serde_derive 1.0.99 (registry+https://github.com/rust-lang/crates.io-index)",
            ]

            [[package]]
            name = "serde_derive"
            version = "1.0.99"
            source = "registry+https://github.com/rust-lang/crates.io-index"
            dependencies = [
             "proc-macro2 1.0.2 (registry+https://github.com/rust-lang/crates.io-index)",
             "quote 1.0.2 (registry+https://github.com/rust-lang/crates.io-index)",
             "syn 1.0.5 (registry+https://github.com/rust-lang/crates.io-index)",
            ]

            [[package]]
            name = "syn"
            version = "1.0.5"
            source = "registry+https://github.com/rust-lang/crates.io-index"
            dependencies = [
             "proc-macro2 1.0.2 (registry+https://github.com/rust-lang/crates.io-index)",
             "quote 1.0.2 (registry+https://github.com/rust-lang/crates.io-index)",
             "unicode-xid 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)",
            ]

            [[package]]
            name = "unicode-xid"
            version = "0.2.0"
            source = "registry+https://github.com/rust-lang/crates.io-index"

            [metadata]
            "checksum proc-macro2 1.0.2 (registry+https://github.com/rust-lang/crates.io-index)" = "175a40b9cf564ce9bf050654633dbf339978706b8ead1a907bb970b63185dd95"
            "checksum quote 1.0.2 (registry+https://github.com/rust-lang/crates.io-index)" = "053a8c8bcc71fcce321828dc897a98ab9760bef03a4fc36693c231e5b3216cfe"
            "checksum serde 1.0.99 (registry+https://github.com/rust-lang/crates.io-index)" = "fec2851eb56d010dc9a21b89ca53ee75e6528bab60c11e89d38390904982da9f"
            "checksum serde_derive 1.0.99 (registry+https://github.com/rust-lang/crates.io-index)" = "cb4dc18c61206b08dc98216c98faa0232f4337e1e1b8574551d5bad29ea1b425"
            "checksum syn 1.0.5 (registry+https://github.com/rust-lang/crates.io-index)" = "66850e97125af79138385e9b88339cbcd037e3f28ceab8c5ad98e64f0f1f80bf"
            "checksum unicode-xid 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)" = "826e7639553986605ec5979c7dd957c7895e93eabed50ab2ffa7f6128a75097c"
        "#;

        let old: Lockfile = old.parse().unwrap();
        let new: Lockfile = new.parse().unwrap();

        let diff = DiffOptions::default().diff(&old, &new);

        serde_json::to_string(&diff).unwrap();
    }
}
