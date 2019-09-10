use crate::Lockfile;
use crate::PackageId;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct DiffOptions;

impl DiffOptions {
    pub fn diff(&self, old_lockfile: &Lockfile, new_lockfile: &Lockfile) -> Diff {
        let mut new = new_lockfile.packages().clone();

        let mut removed = Vec::new();
        for (pkg_id, _pkg) in old_lockfile.packages() {
            if new.remove(pkg_id).is_none() {
                removed.push(pkg_id.clone());
            }
        }
        removed.sort_by(|a, b| a.name().cmp(b.name()));

        let mut added = new
            .into_iter()
            .map(|(pkg_id, _pkg)| pkg_id)
            .collect::<Vec<_>>();
        added.sort_by(|a, b| a.name().cmp(b.name()));

        let duplicates_added = added
            .iter()
            .filter_map(|added_pkg_id| {
                let existing_packages = new_lockfile
                    .packages()
                    .iter()
                    .filter(|(pkg_id, _)| {
                        (*pkg_id != added_pkg_id) && (pkg_id.name() == added_pkg_id.name())
                    })
                    .map(|(pkg_id, _)| pkg_id.clone())
                    .collect::<Vec<_>>();

                if !existing_packages.is_empty() {
                    Some((added_pkg_id.clone(), existing_packages))
                } else {
                    None
                }
            })
            .collect::<HashMap<_, _>>();

        let updated = removed
            .iter()
            .filter_map(|removed_pkg_id| {
                if let Some(updated) = added
                    .iter()
                    .find(|added_pkg_id| removed_pkg_id.name() == added_pkg_id.name())
                {
                    Some((removed_pkg_id.clone(), updated.clone()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        Diff {
            updated,
            removed,
            added,
            duplicates_added,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Diff {
    updated: Vec<(PackageId, PackageId)>,
    removed: Vec<PackageId>,
    added: Vec<PackageId>,
    duplicates_added: HashMap<PackageId, Vec<PackageId>>,
}

impl ::std::fmt::Display for Diff {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
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

        if !self.removed.is_empty() {
            writeln!(f, "Removed Packages:")?;
            for removed in &self.removed {
                writeln!(f, "\t{} {}", removed.name(), removed.version(),)?;
            }
            writeln!(f)?;
        }

        if !self.added.is_empty() {
            writeln!(f, "Added Packages:")?;
            for added in &self.added {
                writeln!(f, "\t{} {}", added.name(), added.version(),)?;
            }
            writeln!(f)?;
        }

        if !self.duplicates_added.is_empty() {
            writeln!(f, "Duplicate Packages Added:")?;
            let mut sorted = self.duplicates_added.iter().collect::<Vec<_>>();
            sorted.sort_by(|(a, _), (b, _)| a.name().cmp(b.name()));
            for (added, existing) in sorted {
                write!(f, "\t{} {}", added.name(), added.version())?;
                write!(f, " ({}", existing[0].version())?;
                for p in &existing[1..] {
                    write!(f, ", {}", p.version())?;
                }
                writeln!(f, ")")?;
            }
        }

        Ok(())
    }
}
