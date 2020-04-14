// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::errors::FeatureBuildStage;
use crate::graph::{
    kind_str, DependencyDirection, EnabledStatus, PackageEdge, PackageGraph, PackageMetadata,
    UnknownStatus, Workspace,
};
use crate::unit_tests::dep_helpers::{
    assert_all_links, assert_deps_internal, assert_topo_ids, assert_topo_metadatas,
    assert_transitive_deps_internal,
};
use crate::{errors::FeatureGraphWarning, DependencyKind, PackageId, Platform};
use once_cell::sync::Lazy;
use pretty_assertions::assert_eq;
use semver::Version;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use target_spec::TargetFeatures;

// Metadata along with interesting crate names.
pub(crate) static METADATA1: &str = include_str!("../../fixtures/small/metadata1.json");
pub(crate) static METADATA1_TESTCRATE: &str = "testcrate 0.1.0 (path+file:///fakepath/testcrate)";
pub(crate) static METADATA1_DATATEST: &str =
    "datatest 0.4.2 (registry+https://github.com/rust-lang/crates.io-index)";
pub(crate) static METADATA1_REGION: &str =
    "region 2.1.2 (registry+https://github.com/rust-lang/crates.io-index)";
pub(crate) static METADATA1_DTOA: &str =
    "dtoa 0.4.4 (registry+https://github.com/rust-lang/crates.io-index)";

pub(crate) static METADATA2: &str = include_str!("../../fixtures/small/metadata2.json");
pub(crate) static METADATA2_TESTCRATE: &str =
    "testworkspace-crate 0.1.0 (path+file:///Users/fakeuser/local/testworkspace/testcrate)";
pub(crate) static METADATA2_WALKDIR: &str =
    "walkdir 2.2.9 (path+file:///Users/fakeuser/local/testworkspace/walkdir)";
pub(crate) static METADATA2_QUOTE: &str = "quote 1.0.2 (path+file:///Users/fakeuser/local/quote)";

pub(crate) static METADATA_DUPS: &str = include_str!("../../fixtures/small/metadata_dups.json");
pub(crate) static METADATA_DUPS_TESTCRATE: &str =
    "testcrate-dups 0.1.0 (path+file:///Users/fakeuser/local/testcrates/testcrate-dups)";
pub(crate) static METADATA_DUPS_LAZY_STATIC_1: &str =
    "lazy_static 1.4.0 (registry+https://github.com/rust-lang/crates.io-index)";
pub(crate) static METADATA_DUPS_LAZY_STATIC_02: &str =
    "lazy_static 0.2.11 (registry+https://github.com/rust-lang/crates.io-index)";
pub(crate) static METADATA_DUPS_BYTES_03: &str =
    "bytes 0.3.0 (registry+https://github.com/rust-lang/crates.io-index)";
pub(crate) static METADATA_DUPS_BYTES_05: &str =
    "bytes 0.5.4 (registry+https://github.com/rust-lang/crates.io-index)";

pub(crate) static METADATA_CYCLE1: &str = include_str!("../../fixtures/small/metadata_cycle1.json");
pub(crate) static METADATA_CYCLE1_BASE: &str =
    "testcycles-base 0.1.0 (path+file:///Users/fakeuser/local/testcrates/testcycles/testcycles-base)";
pub(crate) static METADATA_CYCLE1_HELPER: &str =
    "testcycles-helper 0.1.0 (path+file:///Users/fakeuser/local/testcrates/testcycles/testcycles-helper)";

pub(crate) static METADATA_CYCLE2: &str = include_str!("../../fixtures/small/metadata_cycle2.json");
pub(crate) static METADATA_CYCLE2_UPPER_A: &str =
    "upper-a 0.1.0 (path+file:///Users/fakeuser/local/testcrates/cycle2/upper-a)";
pub(crate) static METADATA_CYCLE2_UPPER_B: &str =
    "upper-b 0.1.0 (path+file:///Users/fakeuser/local/testcrates/cycle2/upper-b)";
pub(crate) static METADATA_CYCLE2_LOWER_A: &str =
    "lower-a 0.1.0 (path+file:///Users/fakeuser/local/testcrates/cycle2/lower-a)";
pub(crate) static METADATA_CYCLE2_LOWER_B: &str =
    "lower-b 0.1.0 (path+file:///Users/fakeuser/local/testcrates/cycle2/lower-b)";

pub(crate) static METADATA_TARGETS1: &str =
    include_str!("../../fixtures/small/metadata_targets1.json");
pub(crate) static METADATA_TARGETS1_TESTCRATE: &str =
    "testcrate-targets 0.1.0 (path+file:///Users/fakeuser/local/testcrates/testcrate-targets)";
pub(crate) static METADATA_TARGETS1_LAZY_STATIC_1: &str =
    "lazy_static 1.4.0 (registry+https://github.com/rust-lang/crates.io-index)";
pub(crate) static METADATA_TARGETS1_LAZY_STATIC_02: &str =
    "lazy_static 0.2.11 (registry+https://github.com/rust-lang/crates.io-index)";
pub(crate) static METADATA_TARGETS1_LAZY_STATIC_01: &str =
    "lazy_static 0.1.16 (registry+https://github.com/rust-lang/crates.io-index)";
pub(crate) static METADATA_TARGETS1_BYTES: &str =
    "bytes 0.5.3 (registry+https://github.com/rust-lang/crates.io-index)";
pub(crate) static METADATA_TARGETS1_DEP_A: &str =
    "dep-a 0.1.0 (path+file:///Users/fakeuser/local/testcrates/dep-a)";

pub(crate) static METADATA_LIBRA: &str = include_str!("../../fixtures/large/metadata_libra.json");
pub(crate) static METADATA_LIBRA_ADMISSION_CONTROL_SERVICE: &str =
    "admission-control-service 0.1.0 (path+file:///Users/fakeuser/local/libra/admission_control/admission-control-service)";
pub(crate) static METADATA_LIBRA_COMPILER: &str =
    "compiler 0.1.0 (path+file:///Users/fakeuser/local/libra/language/compiler)";
pub(crate) static METADATA_LIBRA_E2E_TESTS: &str =
    "language-e2e-tests 0.1.0 (path+file:///Users/fakeuser/local/libra/language/e2e-tests)";
pub(crate) static METADATA_LIBRA_EXECUTOR: &str =
    "executor 0.1.0 (path+file:///Users/fakeuser/local/libra/execution/executor)";
pub(crate) static METADATA_LIBRA_EXECUTOR_UTILS: &str =
    "executor-utils 0.1.0 (path+file:///Users/fakeuser/local/libra/execution/executor-utils)";
pub(crate) static METADATA_LIBRA_COST_SYNTHESIS: &str =
    "cost-synthesis 0.1.0 (path+file:///Users/fakeuser/local/libra/language/tools/cost-synthesis)";
pub(crate) static METADATA_LIBRA_FUNCTIONAL_TESTS: &str =
    "functional_tests 0.1.0 (path+file:///Users/fakeuser/local/libra/language/functional_tests)";
pub(crate) static METADATA_LIBRA_FUNCTIONAL_HYPHEN_TESTS: &str =
    "functional-tests 0.1.0 (path+file:///Users/fakeuser/local/libra/language/functional-tests)";
pub(crate) static METADATA_LIBRA_LIBRA_VM: &str =
    "libra-vm 0.1.0 (path+file:///Users/fakeuser/local/libra/language/libra-vm)";
pub(crate) static METADATA_LIBRA_MOVE_LANG: &str =
    "move-lang 0.0.1 (path+file:///Users/fakeuser/local/libra/language/move-lang)";
pub(crate) static METADATA_LIBRA_MOVE_LANG_STDLIB: &str =
    "move-lang-stdlib 0.1.0 (path+file:///Users/fakeuser/local/libra/language/move-lang/stdlib)";
pub(crate) static METADATA_LIBRA_MOVE_VM_RUNTIME: &str =
    "move-vm-runtime 0.1.0 (path+file:///Users/fakeuser/local/libra/language/move-vm/runtime)";
pub(crate) static METADATA_LIBRA_STDLIB: &str =
    "stdlib 0.1.0 (path+file:///Users/fakeuser/local/libra/language/stdlib)";
pub(crate) static METADATA_LIBRA_TEST_GENERATION: &str =
    "test-generation 0.1.0 (path+file:///Users/fakeuser/local/libra/language/tools/test-generation)";
pub(crate) static METADATA_LIBRA_TRANSACTION_BUILDER: &str =
    "transaction-builder 0.1.0 (path+file:///Users/fakeuser/local/libra/language/transaction-builder)";
pub(crate) static METADATA_LIBRA_VM_GENESIS: &str =
    "vm-genesis 0.1.0 (path+file:///Users/fakeuser/local/libra/language/tools/vm-genesis)";
pub(crate) static METADATA_LIBRA_LANGUAGE_BENCHMARKS: &str =
    "language_benchmarks 0.1.0 (path+file:///Users/fakeuser/local/libra/language/benchmarks)";
pub(crate) static METADATA_LIBRA_TREE_HEAP: &str =
    "tree_heap 0.1.0 (path+file:///Users/fakeuser/local/libra/language/stackless-bytecode/tree_heap)";
pub(crate) static METADATA_LIBRA_LAZY_STATIC: &str =
    "lazy_static 1.4.0 (registry+https://github.com/rust-lang/crates.io-index)";
pub(crate) static METADATA_LIBRA_BACKTRACE: &str =
    "backtrace 0.3.37 (registry+https://github.com/rust-lang/crates.io-index)";
pub(crate) static METADATA_LIBRA_CFG_IF: &str =
    "cfg-if 0.1.9 (registry+https://github.com/rust-lang/crates.io-index)";

pub(crate) static METADATA_LIBRA_F0091A4: &str =
    include_str!("../../fixtures/large/metadata_libra_f0091a4.json");

pub(crate) static METADATA_LIBRA_9FFD93B: &str =
    include_str!("../../fixtures/large/metadata_libra_9ffd93b.json");

pub(crate) static FAKE_AUTHOR: &str = "Fake Author <fakeauthor@example.com>";

macro_rules! define_fixture {
    ($name: ident, $json: ident) => {
        pub(crate) fn $name() -> &'static Fixture {
            static FIXTURE: Lazy<Fixture> = Lazy::new(|| Fixture {
                graph: Fixture::parse_graph($json),
                details: FixtureDetails::$name(),
            });
            &*FIXTURE
        }
    };
}

pub(crate) struct Fixture {
    graph: PackageGraph,
    details: FixtureDetails,
}

impl Fixture {
    /// Returns the package graph for this fixture.
    pub(crate) fn graph(&self) -> &PackageGraph {
        &self.graph
    }

    /// Returns a mutable reference to the package graph for this fixture.
    #[allow(dead_code)]
    pub(crate) fn graph_mut(&mut self) -> &mut PackageGraph {
        &mut self.graph
    }

    /// Returns the test details for this fixture.
    #[allow(dead_code)]
    pub(crate) fn details(&self) -> &FixtureDetails {
        &self.details
    }

    /// Verifies that the parsed metadata matches known details.
    pub(crate) fn verify(&self) {
        self.graph
            .verify()
            .expect("graph verification should succeed");

        self.details.assert_cycles(&self.graph, "cycles");

        self.details.assert_workspace(self.graph.workspace());
        self.details.assert_topo(&self.graph);

        for id in self.details.known_ids() {
            let msg = format!("error while verifying package '{}'", id);
            let metadata = self.graph.metadata(id).expect(&msg);
            self.details.assert_metadata(id, &metadata, &msg);

            // Check for direct dependency queries.
            if self.details.has_deps(id) {
                self.details.assert_deps(&self.graph, id, &msg);
            }
            if self.details.has_reverse_deps(id) {
                self.details.assert_reverse_deps(&self.graph, id, &msg);
            }

            // Check for transitive dependency queries. Use both ID based and edge-based queries.
            if self.details.has_transitive_deps(id) {
                self.details.assert_transitive_deps(
                    &self.graph,
                    id,
                    &format!("{} (transitive deps)", msg),
                );
            }
            if self.details.has_transitive_reverse_deps(id) {
                self.details.assert_transitive_reverse_deps(
                    &self.graph,
                    id,
                    &format!("{} (transitive reverse deps)", msg),
                );
            }

            // Check for named features.
            if self.details.has_named_features(id) {
                self.details.assert_named_features(
                    &self.graph,
                    id,
                    &format!("{} (named features)", msg),
                );
            }
        }

        self.details
            .assert_link_details(&self.graph, "link details");

        // Tests for the feature graph.
        self.details
            .assert_feature_graph_warnings(&self.graph, "feature graph warnings");
    }

    // Specific fixtures follow.

    define_fixture!(metadata1, METADATA1);
    define_fixture!(metadata2, METADATA2);
    define_fixture!(metadata_dups, METADATA_DUPS);
    define_fixture!(metadata_cycle1, METADATA_CYCLE1);
    define_fixture!(metadata_cycle2, METADATA_CYCLE2);
    define_fixture!(metadata_targets1, METADATA_TARGETS1);
    define_fixture!(metadata_libra, METADATA_LIBRA);
    define_fixture!(metadata_libra_f0091a4, METADATA_LIBRA_F0091A4);
    define_fixture!(metadata_libra_9ffd93b, METADATA_LIBRA_9FFD93B);

    fn parse_graph(json: &str) -> PackageGraph {
        let metadata = serde_json::from_str(json).expect("parsing metadata JSON should succeed");
        PackageGraph::new(metadata).expect("constructing package graph should succeed")
    }
}

/// This captures metadata fields that are relevant for tests. They are meant to be written out
/// lazily as tests are filled out -- feel free to add more details as necessary!
pub(crate) struct FixtureDetails {
    workspace_members: Option<BTreeMap<PathBuf, PackageId>>,
    package_details: HashMap<PackageId, PackageDetails>,
    link_details: HashMap<(PackageId, PackageId), LinkDetails>,
    feature_graph_warnings: Vec<FeatureGraphWarning>,
    cycles: Vec<Vec<PackageId>>,
}

impl FixtureDetails {
    pub(crate) fn new(package_details: HashMap<PackageId, PackageDetails>) -> Self {
        Self {
            workspace_members: None,
            package_details,
            link_details: HashMap::new(),
            feature_graph_warnings: vec![],
            cycles: vec![],
        }
    }

    pub(crate) fn with_workspace_members<'a>(
        mut self,
        workspace_members: impl IntoIterator<Item = (impl Into<PathBuf>, &'a str)>,
    ) -> Self {
        self.workspace_members = Some(
            workspace_members
                .into_iter()
                .map(|(path, id)| (path.into(), package_id(id)))
                .collect(),
        );
        self
    }

    pub(crate) fn with_link_details<'a>(
        mut self,
        link_details: HashMap<(PackageId, PackageId), LinkDetails>,
    ) -> Self {
        self.link_details = link_details;
        self
    }

    pub(crate) fn with_feature_graph_warnings(
        mut self,
        mut warnings: Vec<FeatureGraphWarning>,
    ) -> Self {
        warnings.sort();
        self.feature_graph_warnings = warnings;
        self
    }

    pub(crate) fn with_cycles(mut self, cycles: Vec<Vec<&'static str>>) -> Self {
        let mut cycles: Vec<_> = cycles
            .into_iter()
            .map(|cycle| {
                let mut cycle: Vec<_> = cycle.into_iter().map(package_id).collect();
                cycle.sort();
                cycle
            })
            .collect();
        cycles.sort();
        self.cycles = cycles;
        self
    }

    pub(crate) fn known_ids<'a>(&'a self) -> impl Iterator<Item = &'a PackageId> + 'a {
        self.package_details.keys()
    }

    pub(crate) fn assert_workspace(&self, workspace: Workspace) {
        if let Some(expected_members) = &self.workspace_members {
            let members: Vec<_> = workspace.members().into_iter().collect();
            assert_eq!(
                expected_members
                    .iter()
                    .map(|(path, id)| (path.as_path(), id))
                    .collect::<Vec<_>>(),
                members,
                "workspace members should be correct"
            );
        }
    }

    pub(crate) fn assert_topo(&self, graph: &PackageGraph) {
        assert_topo_ids(graph, DependencyDirection::Forward, "topo sort");
        assert_topo_ids(graph, DependencyDirection::Reverse, "reverse topo sort");
        assert_topo_metadatas(graph, DependencyDirection::Forward, "topo sort (metadatas)");
        assert_topo_metadatas(
            graph,
            DependencyDirection::Reverse,
            "reverse topo sort (metadatas)",
        );
        assert_all_links(graph, DependencyDirection::Forward, "all links");
        assert_all_links(graph, DependencyDirection::Reverse, "all links reversed");
    }

    pub(crate) fn assert_metadata(&self, id: &PackageId, metadata: &PackageMetadata, msg: &str) {
        let details = &self.package_details[id];
        details.assert_metadata(metadata, msg);
    }

    // ---
    // Direct dependencies
    // ---

    /// Returns true if the deps for this package are available to test against.
    pub(crate) fn has_deps(&self, id: &PackageId) -> bool {
        let details = &self.package_details[id];
        details.deps.is_some()
    }

    pub(crate) fn assert_deps(&self, graph: &PackageGraph, id: &PackageId, msg: &str) {
        let details = &self.package_details[id];
        assert_deps_internal(&graph, DependencyDirection::Forward, details, msg);
    }

    /// Returns true if the reverse deps for this package are available to test against.
    pub(crate) fn has_reverse_deps(&self, id: &PackageId) -> bool {
        let details = &self.package_details[id];
        details.reverse_deps.is_some()
    }

    pub(crate) fn assert_reverse_deps(&self, graph: &PackageGraph, id: &PackageId, msg: &str) {
        let details = &self.package_details[id];
        assert_deps_internal(&graph, DependencyDirection::Reverse, details, msg);
    }

    // ---
    // Transitive dependencies
    // ---

    /// Returns true if the transitive deps for this package are available to test against.
    pub(crate) fn has_transitive_deps(&self, id: &PackageId) -> bool {
        let details = &self.package_details[id];
        details.transitive_deps.is_some()
    }

    pub(crate) fn assert_transitive_deps<'a>(
        &self,
        graph: &PackageGraph,
        id: &PackageId,
        msg: &str,
    ) {
        assert_transitive_deps_internal(
            graph,
            DependencyDirection::Forward,
            &self.package_details[id],
            msg,
        )
    }

    /// Returns true if the transitive reverse deps for this package are available to test against.
    pub(crate) fn has_transitive_reverse_deps(&self, id: &PackageId) -> bool {
        let details = &self.package_details[id];
        details.transitive_reverse_deps.is_some()
    }

    pub(crate) fn assert_transitive_reverse_deps<'a>(
        &self,
        graph: &PackageGraph,
        id: &PackageId,
        msg: &str,
    ) {
        assert_transitive_deps_internal(
            graph,
            DependencyDirection::Reverse,
            &self.package_details[id],
            msg,
        )
    }

    // ---
    // Links
    // ---

    pub(crate) fn assert_link_details(&self, graph: &PackageGraph, msg: &str) {
        for ((from, to), details) in &self.link_details {
            let mut links: Vec<_> = graph
                .dep_links(from)
                .unwrap_or_else(|| panic!("{}: known package ID '{}' should be valid", msg, from))
                .filter(|link| link.to.id() == to)
                .collect();
            assert_eq!(
                links.len(),
                1,
                "{}: exactly 1 link between '{}' and '{}'",
                msg,
                from,
                to
            );

            let link = links.pop().unwrap();
            let msg = format!("{}: {} -> {}", msg, from, to);
            details.assert_metadata(link.edge, &msg);
        }
    }

    // ---
    // Features
    // ---

    pub(crate) fn has_named_features(&self, id: &PackageId) -> bool {
        self.package_details[id].named_features.is_some()
    }

    pub(crate) fn assert_named_features(&self, graph: &PackageGraph, id: &PackageId, msg: &str) {
        let mut actual: Vec<_> = graph
            .metadata(id)
            .expect("package id should be valid")
            .named_features()
            .collect();
        actual.sort();
        let expected = self.package_details[id].named_features.as_ref().unwrap();
        assert_eq!(expected, &actual, "{}", msg);
    }

    pub(crate) fn assert_feature_graph_warnings(&self, graph: &PackageGraph, msg: &str) {
        let mut actual: Vec<_> = graph.feature_graph().build_warnings().to_vec();
        actual.sort();
        assert_eq!(&self.feature_graph_warnings, &actual, "{}", msg);
    }

    // ---
    // Cycles
    // ---

    pub(crate) fn assert_cycles(&self, graph: &PackageGraph, msg: &str) {
        let mut actual: Vec<_> = graph
            .cycles()
            .all_cycles()
            .map(|cycle| {
                let mut cycle: Vec<_> = cycle.into_iter().collect();
                cycle.sort();
                cycle
            })
            .collect();
        actual.sort();

        assert_eq!(&self.cycles, &actual, "{}", msg);
    }

    // Specific fixtures follow.

    pub(crate) fn metadata1() -> Self {
        let mut details = HashMap::new();

        PackageDetails::new(
            METADATA1_TESTCRATE,
            "testcrate",
            "0.1.0",
            vec![FAKE_AUTHOR],
            None,
            None,
        )
        .with_deps(vec![("datatest", METADATA1_DATATEST)])
        .with_reverse_deps(vec![])
        .insert_into(&mut details);

        PackageDetails::new(
            METADATA1_DATATEST,
            "datatest",
            "0.4.2",
            vec!["Ivan Dubrov <ivan@commure.com>"],
            Some("Data-driven tests in Rust\n"),
            Some("MIT/Apache-2.0"),
        )
        .with_deps(
            vec![
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
            ],
        )
        .with_reverse_deps(vec![("datatest", METADATA1_TESTCRATE)])
        .insert_into(&mut details);

        Self::new(details).with_workspace_members(vec![("", METADATA1_TESTCRATE)])
    }

    pub(crate) fn metadata2() -> Self {
        let mut details = HashMap::new();

        PackageDetails::new(
            METADATA2_TESTCRATE,
            "testworkspace-crate",
            "0.1.0",
            vec![FAKE_AUTHOR],
            None,
            None,
        )
        .with_deps(vec![
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
        ])
        .with_reverse_deps(vec![])
        .insert_into(&mut details);

        PackageDetails::new(
            METADATA2_WALKDIR,
            "walkdir",
            "2.2.9",
            vec![FAKE_AUTHOR],
            None,
            None,
        )
        .with_deps(vec![])
        .with_reverse_deps(vec![("walkdir", METADATA2_TESTCRATE)])
        .insert_into(&mut details);

        // quote was replaced with [patch].
        PackageDetails::new(
            METADATA2_QUOTE,
            "quote",
            "1.0.2",
            vec!["David Tolnay <dtolnay@gmail.com>"],
            Some("Quasi-quoting macro quote!(...)"),
            Some("MIT OR Apache-2.0"),
        )
        .with_deps(vec![(
            "proc-macro2",
            "proc-macro2 1.0.3 (registry+https://github.com/rust-lang/crates.io-index)",
        )])
        .with_reverse_deps(vec![
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
        ])
        .with_named_features(vec!["default", "proc-macro"])
        .insert_into(&mut details);

        Self::new(details).with_workspace_members(vec![
            ("testcrate", METADATA2_TESTCRATE),
            ("walkdir", METADATA2_WALKDIR),
        ])
    }

    pub(crate) fn metadata_dups() -> Self {
        let mut details = HashMap::new();

        PackageDetails::new(
            METADATA_DUPS_TESTCRATE,
            "testcrate-dups",
            "0.1.0",
            vec![FAKE_AUTHOR],
            None,
            None,
        )
        .with_deps(vec![
            ("lazy_static", METADATA_DUPS_LAZY_STATIC_1),
            ("lazy_static", METADATA_DUPS_LAZY_STATIC_02),
            ("bytes-package", METADATA_DUPS_BYTES_03),
            ("bytes-package", METADATA_DUPS_BYTES_05),
        ])
        .insert_into(&mut details);

        Self::new(details).with_workspace_members(vec![("", METADATA_DUPS_TESTCRATE)])
    }

    pub(crate) fn metadata_cycle1() -> Self {
        let mut details = HashMap::new();

        PackageDetails::new(
            METADATA_CYCLE1_BASE,
            "testcycles-base",
            "0.1.0",
            vec![FAKE_AUTHOR],
            None,
            None,
        )
        .with_deps(vec![("testcycles-helper", METADATA_CYCLE1_HELPER)])
        .with_transitive_deps(vec![METADATA_CYCLE1_BASE, METADATA_CYCLE1_HELPER])
        .with_transitive_reverse_deps(vec![METADATA_CYCLE1_BASE, METADATA_CYCLE1_HELPER])
        .insert_into(&mut details);

        PackageDetails::new(
            METADATA_CYCLE1_HELPER,
            "testcycles-helper",
            "0.1.0",
            vec![FAKE_AUTHOR],
            None,
            None,
        )
        .with_deps(vec![("testcycles-base", METADATA_CYCLE1_BASE)])
        .with_transitive_deps(vec![METADATA_CYCLE1_BASE, METADATA_CYCLE1_HELPER])
        .with_transitive_reverse_deps(vec![METADATA_CYCLE1_BASE, METADATA_CYCLE1_HELPER])
        .insert_into(&mut details);

        Self::new(details)
            .with_workspace_members(vec![("", METADATA_CYCLE1_BASE)])
            .with_cycles(vec![vec![METADATA_CYCLE1_BASE, METADATA_CYCLE1_HELPER]])
    }

    pub(crate) fn metadata_cycle2() -> Self {
        // upper-a <-> upper-b
        //                |
        //                v
        //             lower-a <-> lower-b
        let mut details = HashMap::new();

        // upper-a
        PackageDetails::new(
            METADATA_CYCLE2_UPPER_A,
            "upper-a",
            "0.1.0",
            vec![FAKE_AUTHOR],
            None,
            None,
        )
        .with_deps(vec![("upper-b", METADATA_CYCLE2_UPPER_B)])
        .with_reverse_deps(vec![("upper-a", METADATA_CYCLE2_UPPER_B)])
        .with_transitive_deps(vec![
            METADATA_CYCLE2_UPPER_A,
            METADATA_CYCLE2_UPPER_B,
            METADATA_CYCLE2_LOWER_A,
            METADATA_CYCLE2_LOWER_B,
        ])
        .with_transitive_reverse_deps(vec![METADATA_CYCLE2_UPPER_A, METADATA_CYCLE2_UPPER_B])
        .insert_into(&mut details);

        // upper-b
        PackageDetails::new(
            METADATA_CYCLE2_UPPER_B,
            "upper-b",
            "0.1.0",
            vec![FAKE_AUTHOR],
            None,
            None,
        )
        .with_deps(vec![
            ("upper-a", METADATA_CYCLE2_UPPER_A),
            ("lower-a", METADATA_CYCLE2_LOWER_A),
        ])
        .with_reverse_deps(vec![("upper-b", METADATA_CYCLE2_UPPER_A)])
        .with_transitive_deps(vec![
            METADATA_CYCLE2_UPPER_A,
            METADATA_CYCLE2_UPPER_B,
            METADATA_CYCLE2_LOWER_A,
            METADATA_CYCLE2_LOWER_B,
        ])
        .with_transitive_reverse_deps(vec![METADATA_CYCLE2_UPPER_A, METADATA_CYCLE2_UPPER_B])
        .insert_into(&mut details);

        // lower-a
        PackageDetails::new(
            METADATA_CYCLE2_LOWER_A,
            "lower-a",
            "0.1.0",
            vec![FAKE_AUTHOR],
            None,
            None,
        )
        .with_deps(vec![("lower-b", METADATA_CYCLE2_LOWER_B)])
        .with_reverse_deps(vec![
            ("lower-a", METADATA_CYCLE2_UPPER_B),
            ("lower-a", METADATA_CYCLE2_LOWER_B),
        ])
        .with_transitive_deps(vec![METADATA_CYCLE2_LOWER_A, METADATA_CYCLE2_LOWER_B])
        .with_transitive_reverse_deps(vec![
            METADATA_CYCLE2_UPPER_A,
            METADATA_CYCLE2_UPPER_B,
            METADATA_CYCLE2_LOWER_A,
            METADATA_CYCLE2_LOWER_B,
        ])
        .insert_into(&mut details);

        // lower-b
        PackageDetails::new(
            METADATA_CYCLE2_LOWER_B,
            "lower-b",
            "0.1.0",
            vec![FAKE_AUTHOR],
            None,
            None,
        )
        .with_deps(vec![("lower-a", METADATA_CYCLE2_LOWER_A)])
        .with_reverse_deps(vec![("lower-b", METADATA_CYCLE2_LOWER_A)])
        .with_transitive_deps(vec![METADATA_CYCLE2_LOWER_A, METADATA_CYCLE2_LOWER_B])
        .with_transitive_reverse_deps(vec![
            METADATA_CYCLE2_UPPER_A,
            METADATA_CYCLE2_UPPER_B,
            METADATA_CYCLE2_LOWER_A,
            METADATA_CYCLE2_LOWER_B,
        ])
        .insert_into(&mut details);

        Self::new(details)
            .with_workspace_members(vec![
                ("upper-a", METADATA_CYCLE2_UPPER_A),
                ("upper-b", METADATA_CYCLE2_UPPER_B),
                ("lower-a", METADATA_CYCLE2_LOWER_A),
                ("lower-b", METADATA_CYCLE2_LOWER_B),
            ])
            .with_cycles(vec![
                vec![METADATA_CYCLE2_UPPER_A, METADATA_CYCLE2_UPPER_B],
                vec![METADATA_CYCLE2_LOWER_A, METADATA_CYCLE2_LOWER_B],
            ])
    }

    pub(crate) fn metadata_targets1() -> Self {
        // In the testcrate:
        //
        // ```
        // [dependencies]
        // lazy_static = "1"
        // bytes = { version = "0.5", default-features = false, features = ["serde"] }
        // dep-a = { path = "../dep-a", optional = true }
        //
        // [target.'cfg(not(windows))'.dependencies]
        // lazy_static = "0.2"
        // dep-a = { path = "../dep-a", features = ["foo"] }
        //
        // [target.'cfg(windows)'.dev-dependencies]
        // lazy_static = "0.1"
        //
        // [target.'cfg(target_arch = "x86")'.dependencies]
        // bytes = { version = "=0.5.3", optional = false }
        // dep-a = { path = "../dep-a", features = ["bar"] }
        //
        // [target.x86_64-unknown-linux-gnu.build-dependencies]
        // bytes = { version = "0.5.2", optional = true, default-features = false, features = ["std"] }
        //
        // # Platform-specific dev-dependencies.
        //
        // [target.'cfg(any(target_feature = "sse2", target_feature = "atomics"))'.dev-dependencies]
        // dep-a = { path = "../dep-a", default-features = false, features = ["baz"] }
        //
        // [target.'cfg(all(unix, not(target_feature = "sse")))'.dev-dependencies]
        // dep-a = { path = "../dep-a" }
        //
        // [target.'cfg(any(unix, target_feature = "sse"))'.dev-dependencies]
        // dep-a = { path = "../dep-a", default-features = false, features = ["quux"] }
        //
        // # Platform-specific build dependencies.
        //
        // [target.'cfg(target_feature = "sse")'.build-dependencies]
        // dep-a = { path = "../dep-a", default-features = false, features = ["foo"] }
        //
        // # any -- evaluates to true for unix.
        // [target.'cfg(any(unix, target_feature = "sse"))'.build-dependencies]
        // dep-a = { path = "../dep-a", optional = true, default-features = true }
        //
        // # all -- evaluates to unknown on unixes if the target features are unknown.
        // # Evaluates to false on Windows whether target features are known or not.
        // [target.'cfg(all(unix, target_feature = "sse"))'.build-dependencies]
        // dep-a = { path = "../dep-a", optional = true, default-features = false, features = ["bar"] }
        // ```
        let mut details = HashMap::new();

        PackageDetails::new(
            METADATA_TARGETS1_TESTCRATE,
            "testcrate-targets",
            "0.1.0",
            vec![FAKE_AUTHOR],
            None,
            None,
        )
        .with_deps(vec![
            ("lazy_static", METADATA_TARGETS1_LAZY_STATIC_1),
            ("lazy_static", METADATA_TARGETS1_LAZY_STATIC_02),
            ("lazy_static", METADATA_TARGETS1_LAZY_STATIC_01),
            ("bytes", METADATA_TARGETS1_BYTES),
            ("dep-a", METADATA_TARGETS1_DEP_A),
        ])
        .insert_into(&mut details);

        let x86_64_linux =
            Platform::new("x86_64-unknown-linux-gnu", TargetFeatures::Unknown).unwrap();
        let i686_windows = Platform::new(
            "i686-pc-windows-msvc",
            TargetFeatures::features(&["sse", "sse2"]),
        )
        .unwrap();
        let x86_64_windows =
            Platform::new("x86_64-pc-windows-msvc", TargetFeatures::Unknown).unwrap();

        let mut link_details = HashMap::new();

        // testcrate -> lazy_static 1.
        LinkDetails::new(
            package_id(METADATA_TARGETS1_TESTCRATE),
            package_id(METADATA_TARGETS1_LAZY_STATIC_1),
        )
        .with_platform_status(
            DependencyKind::Normal,
            x86_64_linux.clone(),
            PlatformStatus::new(EnabledStatus::Always, EnabledStatus::Always),
        )
        .with_platform_status(
            DependencyKind::Normal,
            i686_windows.clone(),
            PlatformStatus::new(EnabledStatus::Always, EnabledStatus::Always),
        )
        .insert_into(&mut link_details);

        // testcrate -> lazy_static 0.2.
        // Included on not-Windows.
        LinkDetails::new(
            package_id(METADATA_TARGETS1_TESTCRATE),
            package_id(METADATA_TARGETS1_LAZY_STATIC_02),
        )
        .with_platform_status(
            DependencyKind::Normal,
            x86_64_linux.clone(),
            PlatformStatus::new(EnabledStatus::Always, EnabledStatus::Always),
        )
        .with_platform_status(
            DependencyKind::Normal,
            i686_windows.clone(),
            PlatformStatus::new(EnabledStatus::Never, EnabledStatus::Never),
        )
        .insert_into(&mut link_details);

        // testcrate -> lazy_static 0.1.
        // Included as a dev-dependency on Windows.
        LinkDetails::new(
            package_id(METADATA_TARGETS1_TESTCRATE),
            package_id(METADATA_TARGETS1_LAZY_STATIC_01),
        )
        .with_platform_status(
            DependencyKind::Development,
            x86_64_linux.clone(),
            PlatformStatus::new(EnabledStatus::Never, EnabledStatus::Never),
        )
        .with_platform_status(
            DependencyKind::Development,
            i686_windows.clone(),
            PlatformStatus::new(EnabledStatus::Always, EnabledStatus::Always),
        )
        .insert_into(&mut link_details);

        // testcrate -> bytes.
        // As a normal dependency, this is always built but default-features varies.
        // As a build dependency, it is only present on Linux.
        LinkDetails::new(
            package_id(METADATA_TARGETS1_TESTCRATE),
            package_id(METADATA_TARGETS1_BYTES),
        )
        .with_platform_status(
            DependencyKind::Normal,
            x86_64_linux.clone(),
            PlatformStatus::new(EnabledStatus::Always, EnabledStatus::Never)
                .with_feature_status("serde", EnabledStatus::Always)
                .with_feature_status("std", EnabledStatus::Never),
        )
        .with_platform_status(
            DependencyKind::Normal,
            i686_windows.clone(),
            PlatformStatus::new(EnabledStatus::Always, EnabledStatus::Always)
                .with_feature_status("serde", EnabledStatus::Always)
                .with_feature_status("std", EnabledStatus::Never),
        )
        .with_features(DependencyKind::Normal, vec!["serde"])
        .with_platform_status(
            DependencyKind::Build,
            x86_64_linux.clone(),
            PlatformStatus::new(EnabledStatus::Optional, EnabledStatus::Never)
                .with_feature_status("serde", EnabledStatus::Never)
                .with_feature_status("std", EnabledStatus::Optional),
        )
        .with_platform_status(
            DependencyKind::Build,
            i686_windows.clone(),
            PlatformStatus::new(EnabledStatus::Never, EnabledStatus::Never)
                .with_feature_status("serde", EnabledStatus::Never)
                .with_feature_status("std", EnabledStatus::Never),
        )
        .with_features(DependencyKind::Build, vec!["std"])
        .insert_into(&mut link_details);

        // testcrate -> dep-a.
        // As a normal dependency, this is optionally built by default, but on not-Windows or on x86
        // it is mandatory.
        // As a dev dependency, it is present if sse2 or atomics are turned on.
        LinkDetails::new(
            package_id(METADATA_TARGETS1_TESTCRATE),
            package_id(METADATA_TARGETS1_DEP_A),
        )
        .with_platform_status(
            DependencyKind::Normal,
            x86_64_linux.clone(),
            PlatformStatus::new(EnabledStatus::Always, EnabledStatus::Always)
                .with_feature_status("foo", EnabledStatus::Always)
                .with_feature_status("bar", EnabledStatus::Never)
                .with_feature_status("baz", EnabledStatus::Never)
                .with_feature_status("quux", EnabledStatus::Never),
        )
        .with_platform_status(
            DependencyKind::Normal,
            i686_windows.clone(),
            PlatformStatus::new(EnabledStatus::Always, EnabledStatus::Always)
                .with_feature_status("foo", EnabledStatus::Never)
                .with_feature_status("bar", EnabledStatus::Always)
                .with_feature_status("baz", EnabledStatus::Never)
                .with_feature_status("quux", EnabledStatus::Never),
        )
        .with_platform_status(
            DependencyKind::Normal,
            x86_64_windows.clone(),
            PlatformStatus::new(EnabledStatus::Optional, EnabledStatus::Optional)
                .with_feature_status("foo", EnabledStatus::Never)
                .with_feature_status("bar", EnabledStatus::Never)
                .with_feature_status("baz", EnabledStatus::Never)
                .with_feature_status("quux", EnabledStatus::Never),
        )
        .with_platform_status(
            DependencyKind::Development,
            x86_64_linux.clone(),
            // x86_64_linux uses TargetFeature::Unknown.
            PlatformStatus::new(
                EnabledStatus::Always,
                EnabledStatus::Unknown(UnknownStatus::Unknown),
            )
            .with_feature_status("foo", EnabledStatus::Never)
            .with_feature_status("bar", EnabledStatus::Never)
            .with_feature_status("baz", EnabledStatus::Unknown(UnknownStatus::Unknown))
            .with_feature_status("quux", EnabledStatus::Always),
        )
        .with_platform_status(
            DependencyKind::Development,
            i686_windows.clone(),
            // i686_windows turns on sse and sse2.
            PlatformStatus::new(EnabledStatus::Always, EnabledStatus::Never)
                .with_feature_status("foo", EnabledStatus::Never)
                .with_feature_status("bar", EnabledStatus::Never)
                .with_feature_status("baz", EnabledStatus::Always)
                .with_feature_status("quux", EnabledStatus::Always),
        )
        .with_platform_status(
            DependencyKind::Development,
            x86_64_windows.clone(),
            // x86_64_windows uses TargetFeatures::Unknown.
            PlatformStatus::new(
                EnabledStatus::Unknown(UnknownStatus::Unknown),
                EnabledStatus::Never,
            )
            .with_feature_status("foo", EnabledStatus::Never)
            .with_feature_status("bar", EnabledStatus::Never)
            .with_feature_status("baz", EnabledStatus::Unknown(UnknownStatus::Unknown))
            .with_feature_status("quux", EnabledStatus::Unknown(UnknownStatus::Unknown)),
        )
        .with_platform_status(
            DependencyKind::Build,
            x86_64_linux.clone(),
            // x86_64_linux uses TargetFeature::Unknown.
            PlatformStatus::new(
                EnabledStatus::Unknown(UnknownStatus::OptionalPresent),
                EnabledStatus::Optional,
            )
            .with_feature_status("foo", EnabledStatus::Unknown(UnknownStatus::Unknown))
            .with_feature_status(
                "bar",
                EnabledStatus::Unknown(UnknownStatus::OptionalUnknown),
            )
            .with_feature_status("baz", EnabledStatus::Never)
            .with_feature_status("quux", EnabledStatus::Never),
        )
        .with_platform_status(
            DependencyKind::Build,
            i686_windows.clone(),
            // i686_windows turns on sse and sse2.
            PlatformStatus::new(EnabledStatus::Always, EnabledStatus::Optional)
                .with_feature_status("foo", EnabledStatus::Always)
                .with_feature_status("bar", EnabledStatus::Never)
                .with_feature_status("baz", EnabledStatus::Never)
                .with_feature_status("quux", EnabledStatus::Never),
        )
        .with_platform_status(
            DependencyKind::Build,
            x86_64_windows.clone(),
            // x86_64_windows uses TargetFeatures::Unknown.
            PlatformStatus::new(
                EnabledStatus::Unknown(UnknownStatus::Unknown),
                EnabledStatus::Unknown(UnknownStatus::OptionalUnknown),
            )
            .with_feature_status("foo", EnabledStatus::Unknown(UnknownStatus::Unknown))
            .with_feature_status("bar", EnabledStatus::Never)
            .with_feature_status("baz", EnabledStatus::Never)
            .with_feature_status("quux", EnabledStatus::Never),
        )
        .insert_into(&mut link_details);

        Self::new(details)
            .with_workspace_members(vec![("", METADATA_TARGETS1_TESTCRATE)])
            .with_link_details(link_details)
    }

    pub(crate) fn metadata_libra() -> Self {
        let mut details = HashMap::new();

        PackageDetails::new(
            METADATA_LIBRA_E2E_TESTS,
            "language-e2e-tests",
            "0.1.0",
            vec!["Libra Association <opensource@libra.org>"],
            Some("Libra language e2e tests"),
            Some("Apache-2.0"),
        )
        .with_transitive_reverse_deps(vec![
            METADATA_LIBRA_E2E_TESTS,
            METADATA_LIBRA_COST_SYNTHESIS,
            METADATA_LIBRA_FUNCTIONAL_TESTS,
            METADATA_LIBRA_TEST_GENERATION,
            METADATA_LIBRA_LANGUAGE_BENCHMARKS,
            METADATA_LIBRA_TREE_HEAP,
        ])
        .insert_into(&mut details);

        PackageDetails::new(
            METADATA_LIBRA_LAZY_STATIC,
            "lazy_static",
            "1.4.0",
            vec!["Marvin Löbel <loebel.marvin@gmail.com>"],
            Some("A macro for declaring lazily evaluated statics in Rust."),
            Some("MIT/Apache-2.0"),
        )
        .with_transitive_deps(vec![
            METADATA_LIBRA_LAZY_STATIC,
            "spin 0.5.2 (registry+https://github.com/rust-lang/crates.io-index)",
            // lazy_static also has doc-comment as a dev-dependency, but that isn't part of the
            // resolved graph so it won't appear here.
        ])
        .insert_into(&mut details);

        #[rustfmt::skip]
        let workspace_members = vec![
            ("admission_control/admission-control-proto", "admission-control-proto 0.1.0 (path+file:///Users/fakeuser/local/libra/admission_control/admission-control-proto)"),
            ("admission_control/admission-control-service", METADATA_LIBRA_ADMISSION_CONTROL_SERVICE),
            ("benchmark", "benchmark 0.1.0 (path+file:///Users/fakeuser/local/libra/benchmark)"),
            ("client", "client 0.1.0 (path+file:///Users/fakeuser/local/libra/client)"),
            ("client/libra_wallet", "libra-wallet 0.1.0 (path+file:///Users/fakeuser/local/libra/client/libra_wallet)"),
            ("common/bounded-executor", "bounded-executor 0.1.0 (path+file:///Users/fakeuser/local/libra/common/bounded-executor)"),
            ("common/channel", "channel 0.1.0 (path+file:///Users/fakeuser/local/libra/common/channel)"),
            ("common/crash-handler", "crash-handler 0.1.0 (path+file:///Users/fakeuser/local/libra/common/crash-handler)"),
            ("common/datatest-stable", "datatest-stable 0.1.0 (path+file:///Users/fakeuser/local/libra/common/datatest-stable)"),
            ("common/debug-interface", "debug-interface 0.1.0 (path+file:///Users/fakeuser/local/libra/common/debug-interface)"),
            ("common/executable-helpers", "executable-helpers 0.1.0 (path+file:///Users/fakeuser/local/libra/common/executable-helpers)"),
            ("common/failure-ext", "libra-failure-ext 0.1.0 (path+file:///Users/fakeuser/local/libra/common/failure-ext)"),
            ("common/failure-ext/failure-macros", "libra-failure-macros 0.1.0 (path+file:///Users/fakeuser/local/libra/common/failure-ext/failure-macros)"),
            ("common/futures-semaphore", "futures-semaphore 0.1.0 (path+file:///Users/fakeuser/local/libra/common/futures-semaphore)"),
            ("common/grpc-helpers", "grpc-helpers 0.1.0 (path+file:///Users/fakeuser/local/libra/common/grpc-helpers)"),
            ("common/lcs", "libra-canonical-serialization 0.1.0 (path+file:///Users/fakeuser/local/libra/common/lcs)"),
            ("common/logger", "libra-logger 0.1.0 (path+file:///Users/fakeuser/local/libra/common/logger)"),
            ("common/metrics", "libra-metrics 0.1.0 (path+file:///Users/fakeuser/local/libra/common/metrics)"),
            ("common/nibble", "libra-nibble 0.1.0 (path+file:///Users/fakeuser/local/libra/common/nibble)"),
            ("common/proptest-helpers", "libra-proptest-helpers 0.1.0 (path+file:///Users/fakeuser/local/libra/common/proptest-helpers)"),
            ("common/prost-ext", "libra-prost-ext 0.1.0 (path+file:///Users/fakeuser/local/libra/common/prost-ext)"),
            ("common/tools", "libra-tools 0.1.0 (path+file:///Users/fakeuser/local/libra/common/tools)"),
            ("config", "libra-config 0.1.0 (path+file:///Users/fakeuser/local/libra/config)"),
            ("config/config-builder", "config-builder 0.1.0 (path+file:///Users/fakeuser/local/libra/config/config-builder)"),
            ("config/generate-keypair", "generate-keypair 0.1.0 (path+file:///Users/fakeuser/local/libra/config/generate-keypair)"),
            ("consensus", "consensus 0.1.0 (path+file:///Users/fakeuser/local/libra/consensus)"),
            ("consensus/consensus-types", "consensus-types 0.1.0 (path+file:///Users/fakeuser/local/libra/consensus/consensus-types)"),
            ("consensus/safety-rules", "safety-rules 0.1.0 (path+file:///Users/fakeuser/local/libra/consensus/safety-rules)"),
            ("crypto/crypto", "libra-crypto 0.1.0 (path+file:///Users/fakeuser/local/libra/crypto/crypto)"),
            ("crypto/crypto-derive", "libra-crypto-derive 0.1.0 (path+file:///Users/fakeuser/local/libra/crypto/crypto-derive)"),
            ("crypto/secret-service", "secret-service 0.1.0 (path+file:///Users/fakeuser/local/libra/crypto/secret-service)"),
            ("executor", "executor 0.1.0 (path+file:///Users/fakeuser/local/libra/executor)"),
            ("language/benchmarks", METADATA_LIBRA_LANGUAGE_BENCHMARKS),
            ("language/bytecode-verifier", "bytecode-verifier 0.1.0 (path+file:///Users/fakeuser/local/libra/language/bytecode-verifier)"),
            ("language/bytecode-verifier/bytecode_verifier_tests", "bytecode_verifier_tests 0.1.0 (path+file:///Users/fakeuser/local/libra/language/bytecode-verifier/bytecode_verifier_tests)"),
            ("language/bytecode-verifier/invalid-mutations", "invalid-mutations 0.1.0 (path+file:///Users/fakeuser/local/libra/language/bytecode-verifier/invalid-mutations)"),
            ("language/compiler", METADATA_LIBRA_COMPILER),
            ("language/compiler/bytecode-source-map", "bytecode-source-map 0.1.0 (path+file:///Users/fakeuser/local/libra/language/compiler/bytecode-source-map)"),
            ("language/compiler/ir-to-bytecode", "ir-to-bytecode 0.1.0 (path+file:///Users/fakeuser/local/libra/language/compiler/ir-to-bytecode)"),
            ("language/compiler/ir-to-bytecode/syntax", "ir-to-bytecode-syntax 0.1.0 (path+file:///Users/fakeuser/local/libra/language/compiler/ir-to-bytecode/syntax)"),
            ("language/e2e-tests", METADATA_LIBRA_E2E_TESTS),
            ("language/functional_tests", METADATA_LIBRA_FUNCTIONAL_TESTS),
            ("language/stackless-bytecode/bytecode-to-boogie", "bytecode-to-boogie 0.1.0 (path+file:///Users/fakeuser/local/libra/language/stackless-bytecode/bytecode-to-boogie)"),
            ("language/stackless-bytecode/generator", "stackless-bytecode-generator 0.1.0 (path+file:///Users/fakeuser/local/libra/language/stackless-bytecode/generator)"),
            ("language/stackless-bytecode/tree_heap", METADATA_LIBRA_TREE_HEAP),
            ("language/stdlib", METADATA_LIBRA_STDLIB),
            ("language/tools/cost-synthesis", METADATA_LIBRA_COST_SYNTHESIS),
            ("language/tools/test-generation", METADATA_LIBRA_TEST_GENERATION),
            ("language/transaction-builder", METADATA_LIBRA_TRANSACTION_BUILDER),
            ("language/vm", "vm 0.1.0 (path+file:///Users/fakeuser/local/libra/language/vm)"),
            ("language/vm/serializer_tests", "serializer_tests 0.1.0 (path+file:///Users/fakeuser/local/libra/language/vm/serializer_tests)"),
            ("language/vm/vm-genesis", "vm-genesis 0.1.0 (path+file:///Users/fakeuser/local/libra/language/vm/vm-genesis)"),
            ("language/vm/vm-runtime", "vm-runtime 0.1.0 (path+file:///Users/fakeuser/local/libra/language/vm/vm-runtime)"),
            ("language/vm/vm-runtime/vm-cache-map", "vm-cache-map 0.1.0 (path+file:///Users/fakeuser/local/libra/language/vm/vm-runtime/vm-cache-map)"),
            ("language/vm/vm-runtime/vm-runtime-types", "vm-runtime-types 0.1.0 (path+file:///Users/fakeuser/local/libra/language/vm/vm-runtime/vm-runtime-types)"),
            ("libra-node", "libra-node 0.1.0 (path+file:///Users/fakeuser/local/libra/libra-node)"),
            ("libra-swarm", "libra-swarm 0.1.0 (path+file:///Users/fakeuser/local/libra/libra-swarm)"),
            ("mempool", "libra-mempool 0.1.0 (path+file:///Users/fakeuser/local/libra/mempool)"),
            ("mempool/mempool-shared-proto", "libra-mempool-shared-proto 0.1.0 (path+file:///Users/fakeuser/local/libra/mempool/mempool-shared-proto)"),
            ("network", "network 0.1.0 (path+file:///Users/fakeuser/local/libra/network)"),
            ("network/memsocket", "memsocket 0.1.0 (path+file:///Users/fakeuser/local/libra/network/memsocket)"),
            ("network/netcore", "netcore 0.1.0 (path+file:///Users/fakeuser/local/libra/network/netcore)"),
            ("network/noise", "noise 0.1.0 (path+file:///Users/fakeuser/local/libra/network/noise)"),
            ("network/socket-bench-server", "socket-bench-server 0.1.0 (path+file:///Users/fakeuser/local/libra/network/socket-bench-server)"),
            ("state-synchronizer", "state-synchronizer 0.1.0 (path+file:///Users/fakeuser/local/libra/state-synchronizer)"),
            ("storage/accumulator", "accumulator 0.1.0 (path+file:///Users/fakeuser/local/libra/storage/accumulator)"),
            ("storage/jellyfish-merkle", "jellyfish-merkle 0.1.0 (path+file:///Users/fakeuser/local/libra/storage/jellyfish-merkle)"),
            ("storage/libradb", "libradb 0.1.0 (path+file:///Users/fakeuser/local/libra/storage/libradb)"),
            ("storage/schemadb", "schemadb 0.1.0 (path+file:///Users/fakeuser/local/libra/storage/schemadb)"),
            ("storage/scratchpad", "scratchpad 0.1.0 (path+file:///Users/fakeuser/local/libra/storage/scratchpad)"),
            ("storage/state-view", "libra-state-view 0.1.0 (path+file:///Users/fakeuser/local/libra/storage/state-view)"),
            ("storage/storage-client", "storage-client 0.1.0 (path+file:///Users/fakeuser/local/libra/storage/storage-client)"),
            ("storage/storage-proto", "storage-proto 0.1.0 (path+file:///Users/fakeuser/local/libra/storage/storage-proto)"),
            ("storage/storage-service", "storage-service 0.1.0 (path+file:///Users/fakeuser/local/libra/storage/storage-service)"),
            ("testsuite", "testsuite 0.1.0 (path+file:///Users/fakeuser/local/libra/testsuite)"),
            ("testsuite/cluster-test", "cluster-test 0.1.0 (path+file:///Users/fakeuser/local/libra/testsuite/cluster-test)"),
            ("testsuite/libra-fuzzer", "libra-fuzzer 0.1.0 (path+file:///Users/fakeuser/local/libra/testsuite/libra-fuzzer)"),
            ("types", "libra-types 0.1.0 (path+file:///Users/fakeuser/local/libra/types)"),
            ("vm-validator", "vm-validator 0.1.0 (path+file:///Users/fakeuser/local/libra/vm-validator)"),
            ("x", "x 0.1.0 (path+file:///Users/fakeuser/local/libra/x)"),
        ];

        Self::new(details)
            .with_workspace_members(workspace_members)
            .with_feature_graph_warnings(vec![
                // See https://github.com/alexcrichton/cfg-if/issues/22 for more.
                FeatureGraphWarning::MissingFeature {
                    stage: FeatureBuildStage::AddNamedFeatureEdges {
                        package_id: package_id(METADATA_LIBRA_BACKTRACE),
                        from_feature: "rustc-dep-of-std".to_string(),
                    },
                    package_id: package_id(METADATA_LIBRA_CFG_IF),
                    feature_name: "rustc-dep-of-std".to_string(),
                },
            ])
    }

    pub(crate) fn metadata_libra_f0091a4() -> Self {
        let details = HashMap::new();

        Self::new(details).with_cycles(vec![vec![
            METADATA_LIBRA_FUNCTIONAL_HYPHEN_TESTS,
            METADATA_LIBRA_E2E_TESTS,
            METADATA_LIBRA_MOVE_LANG,
            METADATA_LIBRA_MOVE_LANG_STDLIB,
            METADATA_LIBRA_VM_GENESIS,
        ]])
    }

    pub(crate) fn metadata_libra_9ffd93b() -> Self {
        let details = HashMap::new();

        Self::new(details).with_cycles(vec![
            vec![
                METADATA_LIBRA_COMPILER,
                METADATA_LIBRA_FUNCTIONAL_HYPHEN_TESTS,
                METADATA_LIBRA_E2E_TESTS,
                METADATA_LIBRA_LIBRA_VM,
                METADATA_LIBRA_MOVE_LANG,
                METADATA_LIBRA_MOVE_VM_RUNTIME,
                METADATA_LIBRA_STDLIB,
                METADATA_LIBRA_TRANSACTION_BUILDER,
                METADATA_LIBRA_VM_GENESIS,
            ],
            vec![METADATA_LIBRA_EXECUTOR, METADATA_LIBRA_EXECUTOR_UTILS],
        ])
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
    deps: Option<Vec<(&'static str, PackageId)>>,
    reverse_deps: Option<Vec<(&'static str, PackageId)>>,
    transitive_deps: Option<Vec<PackageId>>,
    transitive_reverse_deps: Option<Vec<PackageId>>,
    named_features: Option<Vec<&'static str>>,
}

impl PackageDetails {
    fn new(
        id: &'static str,
        name: &'static str,
        version: &'static str,
        authors: Vec<&'static str>,
        description: Option<&'static str>,
        license: Option<&'static str>,
    ) -> Self {
        Self {
            id: package_id(id),
            name,
            version: Version::parse(version).expect("version should be valid"),
            authors,
            description,
            license,
            deps: None,
            reverse_deps: None,
            transitive_deps: None,
            transitive_reverse_deps: None,
            named_features: None,
        }
    }

    fn with_deps(mut self, mut deps: Vec<(&'static str, &'static str)>) -> Self {
        deps.sort();
        self.deps = Some(
            deps.into_iter()
                .map(|(name, id)| (name, package_id(id)))
                .collect(),
        );
        self
    }

    fn with_reverse_deps(mut self, mut reverse_deps: Vec<(&'static str, &'static str)>) -> Self {
        reverse_deps.sort();
        self.reverse_deps = Some(
            reverse_deps
                .into_iter()
                .map(|(name, id)| (name, package_id(id)))
                .collect(),
        );
        self
    }

    fn with_transitive_deps(mut self, mut transitive_deps: Vec<&'static str>) -> Self {
        transitive_deps.sort();
        self.transitive_deps = Some(transitive_deps.into_iter().map(package_id).collect());
        self
    }

    fn with_transitive_reverse_deps(
        mut self,
        mut transitive_reverse_deps: Vec<&'static str>,
    ) -> Self {
        transitive_reverse_deps.sort();
        self.transitive_reverse_deps = Some(
            transitive_reverse_deps
                .into_iter()
                .map(package_id)
                .collect(),
        );
        self
    }

    fn with_named_features(mut self, mut named_features: Vec<&'static str>) -> Self {
        named_features.sort();
        self.named_features = Some(named_features);
        self
    }

    fn insert_into(self, map: &mut HashMap<PackageId, PackageDetails>) {
        map.insert(self.id.clone(), self);
    }

    pub(crate) fn id(&self) -> &PackageId {
        &self.id
    }

    pub(crate) fn deps(
        &self,
        direction: DependencyDirection,
    ) -> Option<&[(&'static str, PackageId)]> {
        match direction {
            DependencyDirection::Forward => self.deps.as_ref().map(|deps| deps.as_slice()),
            DependencyDirection::Reverse => self.reverse_deps.as_ref().map(|deps| deps.as_slice()),
        }
    }

    pub(crate) fn transitive_deps(&self, direction: DependencyDirection) -> Option<&[PackageId]> {
        match direction {
            DependencyDirection::Forward => {
                self.transitive_deps.as_ref().map(|deps| deps.as_slice())
            }
            DependencyDirection::Reverse => self
                .transitive_reverse_deps
                .as_ref()
                .map(|deps| deps.as_slice()),
        }
    }

    pub(crate) fn assert_metadata(&self, metadata: &PackageMetadata, msg: &str) {
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

#[derive(Clone, Debug)]
pub(crate) struct LinkDetails {
    from: PackageId,
    to: PackageId,
    platform_statuses: Vec<(DependencyKind, Platform<'static>, PlatformStatus)>,
    features: Vec<(DependencyKind, Vec<&'static str>)>,
}

impl LinkDetails {
    pub(crate) fn new(from: PackageId, to: PackageId) -> Self {
        Self {
            from,
            to,
            platform_statuses: vec![],
            features: vec![],
        }
    }

    pub(crate) fn with_platform_status(
        mut self,
        dep_kind: DependencyKind,
        platform: Platform<'static>,
        status: PlatformStatus,
    ) -> Self {
        self.platform_statuses.push((dep_kind, platform, status));
        self
    }

    pub(crate) fn with_features(
        mut self,
        dep_kind: DependencyKind,
        mut features: Vec<&'static str>,
    ) -> Self {
        features.sort();
        self.features.push((dep_kind, features));
        self
    }

    pub(crate) fn insert_into(self, map: &mut HashMap<(PackageId, PackageId), Self>) {
        map.insert((self.from.clone(), self.to.clone()), self);
    }

    pub(crate) fn assert_metadata(&self, edge: &PackageEdge, msg: &str) {
        for (dep_kind, platform, status) in &self.platform_statuses {
            let metadata = edge.metadata_for_kind(*dep_kind).unwrap_or_else(|| {
                panic!(
                    "{}: dependency metadata not found for kind {}",
                    msg,
                    kind_str(*dep_kind)
                )
            });
            assert_eq!(
                metadata.enabled_on(platform),
                status.enabled,
                "{}: for platform '{}', kind {}, enabled is correct",
                msg,
                platform.triple(),
                kind_str(*dep_kind),
            );
            assert_eq!(
                metadata.default_features_on(platform),
                status.default_features,
                "{}: for platform '{}', kind {}, default features is correct",
                msg,
                platform.triple(),
                kind_str(*dep_kind),
            );
            for (feature, status) in &status.feature_statuses {
                assert_eq!(
                    metadata.feature_enabled_on(feature, platform),
                    *status,
                    "{}: for platform '{}', kind {}, feature '{}' has correct status",
                    msg,
                    platform.triple(),
                    kind_str(*dep_kind),
                    feature
                );
            }
        }

        for (dep_kind, features) in &self.features {
            let metadata = edge.metadata_for_kind(*dep_kind).unwrap_or_else(|| {
                panic!(
                    "{}: dependency metadata not found for kind {}",
                    msg,
                    kind_str(*dep_kind)
                )
            });
            let mut actual_features: Vec<_> =
                metadata.features().iter().map(|s| s.as_str()).collect();
            actual_features.sort();
            assert_eq!(&actual_features, features, "{}: features is correct", msg);
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PlatformStatus {
    enabled: EnabledStatus,
    default_features: EnabledStatus,
    feature_statuses: HashMap<String, EnabledStatus>,
}

impl PlatformStatus {
    fn new(enabled: EnabledStatus, default_features: EnabledStatus) -> Self {
        Self {
            enabled,
            default_features,
            feature_statuses: HashMap::new(),
        }
    }

    fn with_feature_status(mut self, feature: &str, status: EnabledStatus) -> Self {
        self.feature_statuses.insert(feature.to_string(), status);
        self
    }
}

/// Helper for creating `PackageId` instances in test code.
pub(crate) fn package_id(s: impl Into<Box<str>>) -> PackageId {
    PackageId::new(s)
}
