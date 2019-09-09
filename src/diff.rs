// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::Lockfile;

#[derive(Default)]
pub struct DiffOptions;

impl DiffOptions {
    pub fn diff(&self, old: &Lockfile, new: &Lockfile) {
        let old = old.packages().clone();
        let mut new = new.packages().clone();

        let mut removed = Vec::new();
        for (pkg_id, _pkg) in old {
            if new.remove(&pkg_id).is_none() {
                removed.push(pkg_id);
            }
        }

        let added = new
            .into_iter()
            .map(|(pkg_id, _pkg)| pkg_id)
            .collect::<Vec<_>>();

        for pkg_id in removed {
            println!("-{} {}", pkg_id.name(), pkg_id.version());
        }

        for pkg_id in added {
            println!("+{} {}", pkg_id.name(), pkg_id.version());
        }
    }
}
