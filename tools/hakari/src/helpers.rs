// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use guppy::Version;
use std::fmt;

/// A formatting wrapper that may print out a minimum version that would match the provided version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VersionDisplay<'a> {
    version: &'a Version,
    exact_versions: bool,
}

impl<'a> VersionDisplay<'a> {
    pub(crate) fn new(version: &'a Version, exact_versions: bool) -> Self {
        Self {
            version,
            exact_versions,
        }
    }
}

impl<'a> fmt::Display for VersionDisplay<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.exact_versions || !self.version.pre.is_empty() {
            // Preserve the version exactly.
            write!(f, "{}", self.version)
        } else if self.version.major >= 1 {
            write!(f, "{}", self.version.major)
        } else if self.version.minor >= 1 {
            write!(f, "{}.{}", self.version.major, self.version.minor)
        } else {
            write!(
                f,
                "{}.{}.{}",
                self.version.major, self.version.minor, self.version.patch
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixtures::json::*;
    use guppy::{graph::DependencyDirection, VersionReq};

    #[test]
    fn min_version() {
        let versions = vec![
            ("1.4.0", "1"),
            ("2.8.0", "2"),
            ("0.4.2", "0.4"),
            ("0.0.7", "0.0.7"),
            ("1.4.0-b1", "1.4.0-b1"),
            ("4.2.3+g456", "4"),
        ];

        for (version_str, min) in versions {
            let version = Version::parse(version_str).expect("valid version");
            let version_req = VersionReq::parse(min).expect("valid version req");
            assert!(
                version_req.matches(&version),
                "version req {} should match version {}",
                min,
                version
            );
            assert_eq!(&format!("{}", VersionDisplay::new(&version, false)), min);
            assert_eq!(
                &format!("{}", VersionDisplay::new(&version, true)),
                version_str
            );
        }
    }

    #[test]
    fn min_versions_match() {
        for (&name, fixture) in JsonFixture::all_fixtures() {
            let graph = fixture.graph();
            for package in graph.resolve_all().packages(DependencyDirection::Forward) {
                let version = package.version();
                let min_version = format!("{}", VersionDisplay::new(version, false));
                let version_req = VersionReq::parse(&min_version).expect("valid version req");

                assert!(
                    version_req.matches(version),
                    "for fixture '{}', for package '{}', min version req {} should match version {}",
                    name,
                    package.id(),
                    min_version,
                    version,
                );
            }
        }
    }
}
