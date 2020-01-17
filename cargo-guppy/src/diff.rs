// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use guppy::{graph::PackageMetadata, PackageId};
use serde::{ser::SerializeStruct, Serialize, Serializer};
use std::collections::HashMap;
use std::ops::Deref;

#[derive(Debug, Default)]
pub struct DiffOptions;

impl DiffOptions {
    pub fn diff<'a>(
        &self,
        old_packages: &[&'a PackageMetadata],
        new_packages: &[&'a PackageMetadata],
    ) -> Diff<'a> {
        let mut new: HashMap<&PackageId, Package> =
            new_packages.iter().map(|p| (p.id(), Package(p))).collect();

        let mut removed = old_packages
            .iter()
            .filter_map(|package| {
                if new.remove(package.id()).is_none() {
                    Some(Package(package))
                } else {
                    None
                }
            })
            .map(|removed_package| {
                let remaining_packages = new_packages
                    .iter()
                    .filter_map(|package| {
                        if (package.id() != removed_package.id())
                            && (package.name() == removed_package.name())
                        {
                            Some(Package(package))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                if remaining_packages.is_empty() {
                    (removed_package.id().clone(), (removed_package, None))
                } else {
                    (
                        removed_package.id().clone(),
                        (removed_package, Some(remaining_packages)),
                    )
                }
            })
            .collect::<HashMap<_, _>>();

        let mut added = new
            .into_iter()
            .map(|(added_package_id, added_package)| {
                let existing_packages = new_packages
                    .iter()
                    .filter_map(|package| {
                        if (package.id() != added_package_id)
                            && (package.name() == added_package.name())
                        {
                            Some(Package(package))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                if existing_packages.is_empty() {
                    (added_package_id, (added_package, None))
                } else {
                    (added_package_id, (added_package, Some(existing_packages)))
                }
            })
            .collect::<HashMap<_, _>>();

        let mut updated = removed
            .iter()
            .filter_map(|(_, (removed_package, _remaining_packages))| {
                if let Some((_updated_package_id, (updated_package, _))) =
                    added
                        .iter()
                        .find(|(_added_package_id, (added_package, _))| {
                            removed_package.name() == added_package.name()
                        })
                {
                    Some((removed_package.clone(), updated_package.clone()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        updated.sort_by(|a, b| a.1.name().cmp(b.1.name()));

        // Remove entries from Added and Removed
        for (removed_pkg, added_pkg) in &updated {
            removed.remove(removed_pkg.id());
            added.remove(added_pkg.id());
        }

        let updated = updated
            .iter()
            .cloned()
            .collect::<Vec<(Package<'_>, Package<'_>)>>();
        let mut removed = removed.into_iter().map(|x| x.1).collect::<Vec<_>>();
        removed.sort_by(|(a, _), (b, _)| a.name().cmp(b.name()));
        let mut added = added.into_iter().map(|x| x.1).collect::<Vec<_>>();
        added.sort_by(|(a, _), (b, _)| a.name().cmp(b.name()));

        Diff {
            updated,
            removed,
            added,
        }
    }
}

#[derive(Clone, Debug)]
struct Package<'a>(pub &'a PackageMetadata);

impl<'a> Deref for Package<'a> {
    type Target = PackageMetadata;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a> Serialize for Package<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Package", 3)?;
        state.serialize_field("id", self.0.id())?;
        state.serialize_field("name", self.0.name())?;
        state.serialize_field("version", self.0.version())?;
        state.end()
    }
}

#[derive(Debug, Serialize)]
pub struct Diff<'a> {
    updated: Vec<(Package<'a>, Package<'a>)>,
    removed: Vec<(Package<'a>, Option<Vec<Package<'a>>>)>,
    added: Vec<(Package<'a>, Option<Vec<Package<'a>>>)>,
}

impl<'a> ::std::fmt::Display for Diff<'a> {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        fn write_dups(
            f: &mut ::std::fmt::Formatter<'_>,
            dups: &Option<Vec<Package>>,
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
