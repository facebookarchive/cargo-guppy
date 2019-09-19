// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph_build::{PackageDep, PackageGraph, PackageMetadata};
use cargo_metadata::PackageId;
use semver::Version;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

// Metadata along with interesting crate names.
pub(crate) static METADATA1: &str = include_str!("../../fixtures/metadata1.json");
pub(crate) static METADATA1_TESTCRATE: &str = "testcrate 0.1.0 (path+file:///fakepath/testcrate)";
pub(crate) static METADATA1_DATATEST: &str =
    "datatest 0.4.2 (registry+https://github.com/rust-lang/crates.io-index)";

pub(crate) static METADATA2: &str = include_str!("../../fixtures/metadata2.json");
pub(crate) static METADATA2_TESTCRATE: &str =
    "testworkspace-crate 0.1.0 (path+file:///Users/fakeuser/local/testworkspace/testcrate)";
pub(crate) static METADATA2_WALKDIR: &str =
    "walkdir 2.2.9 (path+file:///Users/fakeuser/local/testworkspace/walkdir)";
pub(crate) static METADATA2_QUOTE: &str = "quote 1.0.2 (path+file:///Users/fakeuser/local/quote)";

pub(crate) static FAKE_AUTHOR: &str = "Fake Author <fakeauthor@example.com>";

pub(crate) struct Fixture {
    graph: PackageGraph,
    details: FixtureDetails,
}

impl Fixture {
    /// Returns the package graph for this fixture.
    pub(crate) fn graph(&self) -> &PackageGraph {
        &self.graph
    }

    /// Returns the test details for this fixture.
    pub(crate) fn details(&self) -> &FixtureDetails {
        &self.details
    }

    /// Verifies that the parsed metadata matches known details.
    pub(crate) fn verify(&self) {
        self.graph
            .verify()
            .expect("graph verification should succeed");

        self.details
            .assert_workspace_members(self.graph.workspace_members());

        for id in self.details.known_ids() {
            let msg = format!("error while verifying package '{}'", id);
            let metadata = self.graph.metadata(id).expect(&msg);
            self.details.assert_metadata(id, &metadata, &msg);

            if self.details.has_deps(id) {
                self.details.assert_deps(id, self.graph.deps(id), &msg);
            }
            if self.details.has_reverse_deps(id) {
                self.details
                    .assert_reverse_deps(id, self.graph.reverse_deps(id), &msg);
            }
        }
    }

    // Specific fixtures follow.

    pub(crate) fn metadata1() -> Self {
        Self {
            graph: Self::parse_graph(METADATA1),
            details: FixtureDetails::metadata1(),
        }
    }

    pub(crate) fn metadata2() -> Self {
        Self {
            graph: Self::parse_graph(METADATA2),
            details: FixtureDetails::metadata2(),
        }
    }

    fn parse_graph(json: &str) -> PackageGraph {
        let metadata = serde_json::from_str(json).expect("parsing metadata JSON should succeed");
        PackageGraph::new(metadata).expect("constructing package graph should succeed")
    }
}

/// This captures metadata fields that are relevant for tests. They are meant to be written out
/// lazily as tests are filled out -- feel free to add more details as necessary!
pub(crate) struct FixtureDetails {
    workspace_members: BTreeMap<PathBuf, PackageId>,
    package_details: HashMap<PackageId, PackageDetails>,
}

impl FixtureDetails {
    pub(crate) fn new<'a>(
        workspace_members: impl IntoIterator<Item = (impl Into<PathBuf>, &'a str)>,
        package_details: HashMap<PackageId, PackageDetails>,
    ) -> Self {
        let workspace_members = workspace_members
            .into_iter()
            .map(|(path, id)| (path.into(), package_id(id)))
            .collect();
        Self {
            workspace_members,
            package_details,
        }
    }

    pub(crate) fn known_ids<'a>(&'a self) -> impl Iterator<Item = &'a PackageId> + 'a {
        self.package_details.keys()
    }

    pub(crate) fn assert_workspace_members<'a>(
        &self,
        members: impl IntoIterator<Item = (&'a Path, &'a PackageId)>,
    ) {
        let members: Vec<_> = members.into_iter().collect();
        assert_eq!(
            self.workspace_members
                .iter()
                .map(|(path, id)| (path.as_path(), id))
                .collect::<Vec<_>>(),
            members,
            "workspace members should be correct"
        );
    }

    pub(crate) fn assert_metadata(&self, id: &PackageId, metadata: &PackageMetadata, msg: &str) {
        let details = &self.package_details[id];
        details.assert_metadata(metadata, msg);
    }

    /// Returns true if the deps for this package are available to test against.
    pub(crate) fn has_deps<'a>(&self, id: &PackageId) -> bool {
        let details = &self.package_details[id];
        details.deps.is_some()
    }

    pub(crate) fn assert_deps<'a>(
        &self,
        id: &PackageId,
        deps: impl IntoIterator<Item = PackageDep<'a>>,
        msg: &str,
    ) {
        let details = &self.package_details[id];
        let expected_dep_ids = details.deps.as_ref().expect("deps should be present");
        let actual_deps: Vec<PackageDep> = deps.into_iter().collect();
        self.assert_deps_internal(true, details, expected_dep_ids.as_slice(), actual_deps, msg);
    }

    /// Returns true if the reverse deps for this package are available to test against.
    pub(crate) fn has_reverse_deps(&self, id: &PackageId) -> bool {
        let details = &self.package_details[id];
        details.reverse_deps.is_some()
    }

    pub(crate) fn assert_reverse_deps<'a>(
        &self,
        id: &PackageId,
        reverse_deps: impl IntoIterator<Item = PackageDep<'a>>,
        msg: &str,
    ) {
        let details = &self.package_details[id];
        let expected_dep_ids = details
            .reverse_deps
            .as_ref()
            .expect("reverse_deps should be present");
        let actual_deps: Vec<PackageDep> = reverse_deps.into_iter().collect();
        self.assert_deps_internal(
            false,
            details,
            expected_dep_ids.as_slice(),
            actual_deps,
            msg,
        );
    }

    fn assert_deps_internal<'a>(
        &self,
        forward: bool,
        known_details: &PackageDetails,
        expected_dep_ids: &[(&str, &str)],
        actual_deps: Vec<PackageDep<'a>>,
        msg: &str,
    ) {
        // Some of the messages are different based on whether we're testing forward deps or reverse
        // ones. For forward deps, we use the terms "known" for 'from' and "variable" for 'to'. For
        // reverse deps it's the other way round.

        fn __from_metadata<'a>(dep: &PackageDep<'a>) -> &'a PackageMetadata {
            dep.from
        }
        fn __to_metadata<'a>(dep: &PackageDep<'a>) -> &'a PackageMetadata {
            dep.to
        }
        type DepToMetadata<'a> = fn(&PackageDep<'a>) -> &'a PackageMetadata;

        let (direction_desc, known_desc, variable_desc, known_metadata, variable_metadata) =
            if forward {
                (
                    "forward",
                    "from",
                    "to",
                    __from_metadata as DepToMetadata<'a>,
                    __to_metadata as DepToMetadata<'a>,
                )
            } else {
                (
                    "reverse",
                    "to",
                    "from",
                    __to_metadata as DepToMetadata<'a>,
                    __from_metadata as DepToMetadata<'a>,
                )
            };

        // Compare (dep_name, resolved_name, id) triples.
        let expected_dep_ids: Vec<_> = expected_dep_ids
            .iter()
            .map(|(dep_name, id)| (*dep_name, dep_name.replace("-", "_"), *id))
            .collect();
        let mut actual_dep_ids: Vec<_> = actual_deps
            .iter()
            .map(|dep| {
                (
                    dep.edge.dep_name(),
                    dep.edge.resolved_name().to_string(),
                    variable_metadata(dep).id().repr.as_str(),
                )
            })
            .collect();
        actual_dep_ids.sort();
        assert_eq!(
            expected_dep_ids, actual_dep_ids,
            "{}: expected {} dependencies",
            msg, direction_desc,
        );

        // Check that the dependency metadata returned is consistent with what we expect.
        let known_msg = format!(
            "{}: {} dependency edge '{}'",
            msg, direction_desc, known_desc
        );
        for actual_dep in &actual_deps {
            known_details.assert_metadata(known_metadata(&actual_dep), &known_msg);
            // The variable metadata might be missing -- only compare it if it's present.
            let variable = variable_metadata(&actual_dep);
            let variable_id = variable.id();
            if let Some(variable_details) = self.package_details.get(variable_id) {
                variable_details.assert_metadata(
                    &variable,
                    &format!(
                        "{}: {} dependency edge '{}': {}",
                        msg, direction_desc, variable_desc, variable_id
                    ),
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
            vec![FAKE_AUTHOR],
            None,
            None,
            Some(vec![("datatest", METADATA1_DATATEST)]),
            Some(vec![]),
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
            Some(vec![("datatest", METADATA1_TESTCRATE)]),
        );

        Self::new(vec![("", METADATA1_TESTCRATE)], details)
    }

    pub(crate) fn metadata2() -> Self {
        let mut details = HashMap::new();

        add(
            &mut details,
            METADATA2_TESTCRATE,
            "testworkspace-crate",
            "0.1.0",
            vec![FAKE_AUTHOR],
            None,
            None,
            Some(vec![
                (
                    "datatest",
                    "datatest 0.4.2 (registry+https://github.com/rust-lang/crates.io-index)",
                ),
                // There are three instances of walkdir in the dependencies -- ensure they all
                // link up correctly.
                ("walkdir", METADATA2_WALKDIR),
                (
                    "walkdir-crates-io",
                    "walkdir 2.2.9 (registry+https://github.com/rust-lang/crates.io-index)",
                ),
                (
                    "walkdir-nuevo",
                    "walkdir 0.1.0 (path+file:///Users/fakeuser/local/walkdir)",
                ),
            ]),
            Some(vec![]),
        );
        add(
            &mut details,
            METADATA2_WALKDIR,
            "walkdir",
            "2.2.9",
            vec![FAKE_AUTHOR],
            None,
            None,
            Some(vec![]),
            Some(vec![("walkdir", METADATA2_TESTCRATE)]),
        );
        // quote was replaced with [patch].
        add(
            &mut details,
            METADATA2_QUOTE,
            "quote",
            "1.0.2",
            vec!["David Tolnay <dtolnay@gmail.com>"],
            Some("Quasi-quoting macro quote!(...)"),
            Some("MIT OR Apache-2.0"),
            Some(vec![(
                "proc-macro2",
                "proc-macro2 1.0.3 (registry+https://github.com/rust-lang/crates.io-index)",
            )]),
            Some(vec![
                (
                    "quote",
                    "ctor 0.1.10 (registry+https://github.com/rust-lang/crates.io-index)",
                ),
                (
                    "quote",
                    "datatest-derive 0.4.0 (registry+https://github.com/rust-lang/crates.io-index)",
                ),
                (
                    "quote",
                    "syn 1.0.5 (registry+https://github.com/rust-lang/crates.io-index)",
                ),
            ]),
        );

        Self::new(
            vec![
                ("testcrate", METADATA2_TESTCRATE),
                ("walkdir", METADATA2_WALKDIR),
            ],
            details,
        )
    }
}

pub(crate) struct PackageDetails {
    id: PackageId,
    name: &'static str,
    version: Version,
    authors: Vec<&'static str>,
    description: Option<&'static str>,
    license: Option<&'static str>,

    // The vector items are (name, package id).
    // XXX add more details about dependency edges here?
    deps: Option<Vec<(&'static str, &'static str)>>,
    reverse_deps: Option<Vec<(&'static str, &'static str)>>,
}

impl PackageDetails {
    fn assert_metadata(&self, metadata: &PackageMetadata, msg: &str) {
        assert_eq!(&self.id, metadata.id(), "{}: same package ID", msg);
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
    map: &mut HashMap<PackageId, PackageDetails>,
    id: &'static str,
    name: &'static str,
    version: &'static str,
    authors: Vec<&'static str>,
    description: Option<&'static str>,
    license: Option<&'static str>,
    deps: Option<Vec<(&'static str, &'static str)>>,
    reverse_deps: Option<Vec<(&'static str, &'static str)>>,
) {
    fn sort_opt<T: Ord>(v: Option<Vec<T>>) -> Option<Vec<T>> {
        v.map(|mut val| {
            val.sort();
            val
        })
    }

    let id = package_id(id);

    map.insert(
        id.clone(),
        PackageDetails {
            id,
            name,
            version: Version::parse(version).expect("version should be valid"),
            authors,
            description,
            license,
            deps: sort_opt(deps),
            reverse_deps: sort_opt(reverse_deps),
        },
    );
}

/// Helper for creating `PackageId` instances in test code.
pub(crate) fn package_id(s: impl Into<String>) -> PackageId {
    PackageId { repr: s.into() }
}
