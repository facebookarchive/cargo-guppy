use crate::Lockfile;
use crate::PackageId;
use serde::Serialize;
use std::collections::HashMap;
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct DiffOptions;

impl DiffOptions {
    pub fn diff(&self, old_lockfile: &Lockfile, new_lockfile: &Lockfile) -> Diff {
        let mut new = new_lockfile.packages().clone();

        let mut removed = HashSet::new();
        for (pkg_id, _pkg) in old_lockfile.packages() {
            if new.remove(pkg_id).is_none() {
                removed.insert(pkg_id.clone());
            }
        }

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
            .filter_map(|removed_pkg_id| {
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
    removed: HashSet<PackageId>,
    added: Vec<(PackageId, Option<Vec<PackageId>>)>,
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
            let mut removed_sorted = self.removed.iter().collect::<Vec<_>>();
            removed_sorted.sort_by(|a, b| a.name().cmp(b.name()));
            for removed in removed_sorted {
                writeln!(f, "\t{} {}", removed.name(), removed.version(),)?;
            }
            writeln!(f)?;
        }

        if !self.added.is_empty() {
            writeln!(f, "Added Packages (existing duplicates in '()'):")?;
            for (added, dups) in &self.added {
                write!(f, "\t{} {}", added.name(), added.version(),)?;

                if let Some(dups) = dups {
                    write!(f, " ({}", dups[0].version())?;
                    for p in &dups[1..] {
                        write!(f, ", {}", p.version())?;
                    }
                    write!(f, ")")?;
                }
                writeln!(f)?;
            }
            writeln!(f)?;
        }

        Ok(())
    }
}
