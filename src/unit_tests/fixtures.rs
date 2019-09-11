// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::{PackageDep, PackageMetadata};
use cargo_metadata::Metadata;
use semver::Version;
use std::collections::HashMap;

// Metadata along with interesting crate names in the m
pub(crate) static METADATA1: &str = include_str!("../../fixtures/metadata1.json");
pub(crate) static METADATA1_TESTCRATE: &str = "testcrate 0.1.0 (path+file:///fakepath/testcrate)";
pub(crate) static METADATA1_DATATEST: &str =
    "datatest 0.4.2 (registry+https://github.com/rust-lang/crates.io-index)";

pub(crate) fn parse_metadata(json: &str) -> Metadata {
    serde_json::from_str(json).expect("parsing metadata JSON should succeed")
}

/// This captures metadata fields that are relevant for tests. They are meant to be written out
/// lazily as tests are filled out -- feel free to add more details as necessary!
pub(crate) struct FixtureDetails {
    details: HashMap<&'static str, PackageDetails>,
}

impl FixtureDetails {
    pub(crate) fn assert_metadata(&self, id: &str, metadata: &PackageMetadata, msg: &str) {
        let details = &self.details[id];
        details.assert_metadata(metadata, msg);
    }

    pub(crate) fn assert_dependencies<'a>(
        &self,
        id: &str,
        deps: impl IntoIterator<Item = PackageDep<'a>>,
        msg: &str,
    ) {
        let details = &self.details[id];
        let expected_dep_ids = details.deps.as_ref().expect("deps should be present");
        let actual_deps: Vec<PackageDep> = deps.into_iter().collect();
        let mut actual_dep_ids: Vec<(&str, &str)> = actual_deps
            .iter()
            .map(|dep| (dep.edge.name(), dep.to.id().repr.as_str()))
            .collect();
        actual_dep_ids.sort();

        assert_eq!(
            expected_dep_ids, &actual_dep_ids,
            "{}: expected dependencies",
            msg
        );

        // Check that the dependency metadata returned is consistent with what we expect.
        let from_msg = format!("{}: dependency 'from'", msg);
        for actual_dep in &actual_deps {
            details.assert_metadata(&actual_dep.from, &from_msg);
            // The 'to' metadata might be missing -- only compare it if it's present.
            let to_id = actual_dep.to.id();
            if let Some(to_details) = self.details.get(to_id.repr.as_str()) {
                to_details.assert_metadata(
                    &actual_dep.to,
                    &format!("{}: dependency from this crate 'to' {}", msg, to_id),
                );
            }
            // XXX maybe compare version requirements?
        }
    }

    // Specific fixtures follow.

    pub(crate) fn metadata1() -> Self {
        let mut details = HashMap::new();

        add(
            &mut details,
            METADATA1_TESTCRATE,
            "testcrate",
            "0.1.0",
            vec!["Fake Author <fakeauthor@example.com>"],
            None,
            None,
            Some(vec![("datatest", METADATA1_DATATEST)]),
        );
        add(
            &mut details,
            METADATA1_DATATEST,
            "datatest",
            "0.4.2",
            vec!["Ivan Dubrov <ivan@commure.com>"],
            Some("Data-driven tests in Rust\n"),
            Some("MIT/Apache-2.0"),
            Some(vec![
                ("ctor", "ctor 0.1.10 (registry+https://github.com/rust-lang/crates.io-index)"),
                ("datatest-derive", "datatest-derive 0.4.0 (registry+https://github.com/rust-lang/crates.io-index)"),
                ("regex", "regex 1.3.1 (registry+https://github.com/rust-lang/crates.io-index)"),
                ("region", "region 2.1.2 (registry+https://github.com/rust-lang/crates.io-index)"),
                ("serde", "serde 1.0.100 (registry+https://github.com/rust-lang/crates.io-index)"),
                ("serde_yaml", "serde_yaml 0.8.9 (registry+https://github.com/rust-lang/crates.io-index)"),
                ("version_check", "version_check 0.9.1 (registry+https://github.com/rust-lang/crates.io-index)"),
                // walkdir was replaced with [replace] (see metadata1.toml) -- ensure that the
                // *replaced* version shows up here, not the regular one.
                ("walkdir", "walkdir 2.2.9 (git+https://github.com/BurntSushi/walkdir?tag=2.2.9#7c7013259eb9db400b3e5c7bc60330ca08068826)"),
                ("yaml-rust", "yaml-rust 0.4.3 (registry+https://github.com/rust-lang/crates.io-index)")
            ]),
        );

        Self { details }
    }
}

pub(crate) struct PackageDetails {
    id: &'static str,
    name: &'static str,
    version: Version,
    authors: Vec<&'static str>,
    description: Option<&'static str>,
    license: Option<&'static str>,

    // The vector items are (name, package id).
    // XXX add more details about dependency edges here?
    deps: Option<Vec<(&'static str, &'static str)>>,
}

impl PackageDetails {
    fn assert_metadata(&self, metadata: &PackageMetadata, msg: &str) {
        assert_eq!(self.id, &metadata.id().repr, "{}: same package ID", msg);
        assert_eq!(self.name, metadata.name(), "{}: same name", msg);
        assert_eq!(&self.version, metadata.version(), "{}: same version", msg);
        assert_eq!(
            &self.authors,
            &metadata
                .authors()
                .iter()
                .map(|author| author.as_str())
                .collect::<Vec<_>>(),
            "{}: same authors",
            msg
        );
        assert_eq!(
            &self.description,
            &metadata.description(),
            "{}: same description",
            msg
        );
        assert_eq!(&self.license, &metadata.license(), "{}: same license", msg);
    }
}

fn add(
    map: &mut HashMap<&'static str, PackageDetails>,
    id: &'static str,
    name: &'static str,
    version: &'static str,
    authors: Vec<&'static str>,
    description: Option<&'static str>,
    license: Option<&'static str>,
    deps: Option<Vec<(&'static str, &'static str)>>,
) {
    map.insert(
        id,
        PackageDetails {
            id,
            name,
            version: Version::parse(version).expect("version should be valid"),
            authors,
            description,
            license,
            deps: deps.map(|mut deps| {
                deps.sort();
                deps
            }),
        },
    );
}
