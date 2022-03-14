// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{
    diff::{changed_sort_key, PackageDiff, SummaryDiff, SummaryDiffStatus},
    SummaryId,
};
use std::fmt;

/// A report of a diff between two summaries.
///
/// This report can be generated or written to a file through `fmt::Display`.
#[derive(Clone, Debug)]
pub struct SummaryReport<'a, 'b> {
    diff: &'b SummaryDiff<'a>,
    sorted_target: Vec<(&'a SummaryId, &'b SummaryDiffStatus<'a>)>,
    sorted_host: Vec<(&'a SummaryId, &'b SummaryDiffStatus<'a>)>,
}

impl<'a, 'b> SummaryReport<'a, 'b> {
    /// Creates a new `SummaryReport` that can be displayed.
    pub fn new(diff: &'b SummaryDiff<'a>) -> Self {
        let sorted_target = Self::make_sorted(&diff.target_packages);
        let sorted_host = Self::make_sorted(&diff.host_packages);

        Self {
            diff,
            sorted_target,
            sorted_host,
        }
    }

    fn make_sorted(
        packages: &'b PackageDiff<'a>,
    ) -> Vec<(&'a SummaryId, &'b SummaryDiffStatus<'a>)> {
        let mut v: Vec<_> = packages
            .changed
            .iter()
            .map(|(summary_id, status)| (*summary_id, status))
            .collect();
        v.sort_by_key(|(summary_id, status)| changed_sort_key(summary_id, status));

        v
    }
}

impl<'a, 'b> fmt::Display for SummaryReport<'a, 'b> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.diff.target_packages.is_unchanged() {
            writeln!(
                f,
                "target packages:\n{}",
                PackageReport::new(&self.diff.target_packages, &self.sorted_target)
            )?;
        }
        if !self.diff.host_packages.is_unchanged() {
            writeln!(
                f,
                "host packages:\n{}",
                PackageReport::new(&self.diff.host_packages, &self.sorted_host)
            )?;
        }

        Ok(())
    }
}

// Collapse the lifetime params into one because three is too annoying, all the params here are
// covariant anyway, and this is an internal struct.
struct PackageReport<'x> {
    package_diff: &'x PackageDiff<'x>,
    sorted: &'x [(&'x SummaryId, &'x SummaryDiffStatus<'x>)],
}

impl<'x> PackageReport<'x> {
    fn new(
        package_diff: &'x PackageDiff<'x>,
        sorted: &'x [(&'x SummaryId, &'x SummaryDiffStatus<'x>)],
    ) -> Self {
        Self {
            package_diff,
            sorted,
        }
    }
}

impl<'x> fmt::Display for PackageReport<'x> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (summary_id, status) in self.sorted {
            write!(
                f,
                "  {} {} {} ({}, {})",
                status.tag(),
                summary_id.name,
                summary_id.version,
                status.latest_status(),
                summary_id.source
            )?;

            // Print out other versions if available.
            if let Some(unchanged_list) = self.package_diff.unchanged.get(summary_id.name.as_str())
            {
                write!(f, " (other versions: ")?;
                display_list(f, unchanged_list.iter().map(|(version, _, _)| *version))?;
                write!(f, ")")?;
            }

            writeln!(f)?;

            match status {
                SummaryDiffStatus::Added { info } => {
                    write!(f, "    * features: ")?;
                    display_list(f, &info.features)?;
                    writeln!(f)?;
                }
                SummaryDiffStatus::Removed { old_info } => {
                    write!(f, "    * (old features: ")?;
                    display_list(f, &old_info.features)?;
                    writeln!(f, ")")?;
                }
                SummaryDiffStatus::Modified {
                    old_version,
                    old_source,
                    old_status,
                    // The new status is printed in the package header.
                    new_status: _,
                    added_features,
                    removed_features,
                    unchanged_features,
                    added_optional_deps,
                    removed_optional_deps,
                    unchanged_optional_deps,
                } => {
                    if let Some(old_version) = old_version {
                        let change_str = if summary_id.version > **old_version {
                            "upgraded"
                        } else {
                            "DOWNGRADED"
                        };
                        writeln!(f, "    * version {} from {}", change_str, old_version)?;
                    }
                    if let Some(old_source) = old_source {
                        writeln!(f, "    * source changed from {}", old_source)?;
                    }
                    if let Some(old_status) = old_status {
                        writeln!(f, "    * status changed from {}", old_status)?;
                    }

                    // ---

                    if !added_features.is_empty() {
                        write!(f, "    * added features: ")?;
                        display_list(f, added_features.iter().copied())?;
                        writeln!(f)?;
                    }
                    if !removed_features.is_empty() {
                        write!(f, "    * removed features: ")?;
                        display_list(f, removed_features.iter().copied())?;
                        writeln!(f)?;
                    }
                    write!(f, "    * (unchanged features: ")?;
                    display_list(f, unchanged_features.iter().copied())?;
                    writeln!(f, ")")?;

                    // ---

                    if !added_optional_deps.is_empty() {
                        write!(f, "    * added optional dependencies: ")?;
                        display_list(f, added_optional_deps.iter().copied())?;
                        writeln!(f)?;
                    }
                    if !removed_optional_deps.is_empty() {
                        write!(f, "    * removed optional dependencies: ")?;
                        display_list(f, removed_optional_deps.iter().copied())?;
                        writeln!(f)?;
                    }
                    write!(f, "    * (unchanged optional dependencies: ")?;
                    display_list(f, unchanged_optional_deps.iter().copied())?;
                    writeln!(f, ")")?;
                }
            }
        }

        Ok(())
    }
}

fn display_list<I>(f: &mut fmt::Formatter, items: I) -> fmt::Result
where
    I: IntoIterator,
    I::Item: fmt::Display,
    I::IntoIter: ExactSizeIterator,
{
    let items = items.into_iter();
    let len = items.len();
    if len == 0 {
        write!(f, "[none]")?;
    }

    for (idx, item) in items.enumerate() {
        write!(f, "{}", item)?;
        // Add a comma for all items except the last one.
        if idx + 1 < len {
            write!(f, ", ")?;
        }
    }

    Ok(())
}
