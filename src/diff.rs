use crate::Lockfile;
use crate::PackageId;

#[derive(Debug, Default)]
pub struct DiffOptions;

impl DiffOptions {
    pub fn diff(&self, old: &Lockfile, new: &Lockfile) -> Diff {
        let mut new = new.packages().clone();

        let mut removed = Vec::new();
        for (pkg_id, _pkg) in old.packages() {
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
        }
    }
}

#[derive(Debug)]
pub struct Diff {
    updated: Vec<(PackageId, PackageId)>,
    removed: Vec<PackageId>,
    added: Vec<PackageId>,
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
        }

        Ok(())
    }
}
