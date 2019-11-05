// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::{DependencyInfo, PackageGraph, PackageMetadata, Workspace};
use crate::unit_tests::dep_helpers::assert_deps_internal;
use cargo_metadata::PackageId;
use semver::Version;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

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

pub(crate) static METADATA_LIBRA: &str = include_str!("../../fixtures/metadata_libra.json");

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

        self.details.assert_workspace(self.graph.workspace());

        for id in self.details.known_ids() {
            let msg = format!("error while verifying package '{}'", id);
            let metadata = self.graph.metadata(id).expect(&msg);
            self.details.assert_metadata(id, &metadata, &msg);

            if self.details.has_deps(id) {
                self.details.assert_deps(
                    id,
                    self.graph
                        .deps(id)
                        .unwrap_or_else(|| panic!("{}: deps for package not found", msg)),
                    &msg,
                );
            }
            if self.details.has_reverse_deps(id) {
                self.details.assert_reverse_deps(
                    id,
                    self.graph
                        .reverse_deps(id)
                        .unwrap_or_else(|| panic!("{}: reverse deps for package not found", msg)),
                    &msg,
                );
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

    pub(crate) fn metadata_libra() -> Self {
        Self {
            graph: Self::parse_graph(METADATA_LIBRA),
            details: FixtureDetails::metadata_libra(),
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

    pub(crate) fn assert_workspace(&self, workspace: &Workspace) {
        let members: Vec<_> = workspace.members().into_iter().collect();
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
    pub(crate) fn has_deps(&self, id: &PackageId) -> bool {
        let details = &self.package_details[id];
        details.deps.is_some()
    }

    pub(crate) fn assert_deps<'a>(
        &self,
        id: &PackageId,
        deps: impl IntoIterator<Item = DependencyInfo<'a>>,
        msg: &str,
    ) {
        let details = &self.package_details[id];
        let expected_dep_ids = details.deps.as_ref().expect("deps should be present");
        let actual_deps: Vec<DependencyInfo> = deps.into_iter().collect();
        assert_deps_internal(true, details, expected_dep_ids.as_slice(), actual_deps, msg);
    }

    /// Returns true if the reverse deps for this package are available to test against.
    pub(crate) fn has_reverse_deps(&self, id: &PackageId) -> bool {
        let details = &self.package_details[id];
        details.reverse_deps.is_some()
    }

    pub(crate) fn assert_reverse_deps<'a>(
        &self,
        id: &PackageId,
        reverse_deps: impl IntoIterator<Item = DependencyInfo<'a>>,
        msg: &str,
    ) {
        let details = &self.package_details[id];
        let expected_dep_ids = details
            .reverse_deps
            .as_ref()
            .expect("reverse_deps should be present");
        let actual_deps: Vec<DependencyInfo> = reverse_deps.into_iter().collect();
        assert_deps_internal(
            false,
            details,
            expected_dep_ids.as_slice(),
            actual_deps,
            msg,
        );
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

        Self::new(vec![("", METADATA1_TESTCRATE)], details)
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
        .insert_into(&mut details);

        Self::new(
            vec![
                ("testcrate", METADATA2_TESTCRATE),
                ("walkdir", METADATA2_WALKDIR),
            ],
            details,
        )
    }

    pub(crate) fn metadata_libra() -> Self {
        Self::new(vec![
            ("admission_control/admission-control-proto", "admission-control-proto 0.1.0 (path+file:///Users/fakeuser/local/libra/admission_control/admission-control-proto)"),
            ("admission_control/admission-control-service", "admission-control-service 0.1.0 (path+file:///Users/fakeuser/local/libra/admission_control/admission-control-service)"),
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
            ("language/benchmarks", "language_benchmarks 0.1.0 (path+file:///Users/fakeuser/local/libra/language/benchmarks)"),
            ("language/bytecode-verifier", "bytecode-verifier 0.1.0 (path+file:///Users/fakeuser/local/libra/language/bytecode-verifier)"),
            ("language/bytecode-verifier/bytecode_verifier_tests", "bytecode_verifier_tests 0.1.0 (path+file:///Users/fakeuser/local/libra/language/bytecode-verifier/bytecode_verifier_tests)"),
            ("language/bytecode-verifier/invalid-mutations", "invalid-mutations 0.1.0 (path+file:///Users/fakeuser/local/libra/language/bytecode-verifier/invalid-mutations)"),
            ("language/compiler", "compiler 0.1.0 (path+file:///Users/fakeuser/local/libra/language/compiler)"),
            ("language/compiler/bytecode-source-map", "bytecode-source-map 0.1.0 (path+file:///Users/fakeuser/local/libra/language/compiler/bytecode-source-map)"),
            ("language/compiler/ir-to-bytecode", "ir-to-bytecode 0.1.0 (path+file:///Users/fakeuser/local/libra/language/compiler/ir-to-bytecode)"),
            ("language/compiler/ir-to-bytecode/syntax", "ir-to-bytecode-syntax 0.1.0 (path+file:///Users/fakeuser/local/libra/language/compiler/ir-to-bytecode/syntax)"),
            ("language/e2e-tests", "language-e2e-tests 0.1.0 (path+file:///Users/fakeuser/local/libra/language/e2e-tests)"),
            ("language/functional_tests", "functional_tests 0.1.0 (path+file:///Users/fakeuser/local/libra/language/functional_tests)"),
            ("language/stackless-bytecode/bytecode-to-boogie", "bytecode-to-boogie 0.1.0 (path+file:///Users/fakeuser/local/libra/language/stackless-bytecode/bytecode-to-boogie)"),
            ("language/stackless-bytecode/generator", "stackless-bytecode-generator 0.1.0 (path+file:///Users/fakeuser/local/libra/language/stackless-bytecode/generator)"),
            ("language/stackless-bytecode/tree_heap", "tree_heap 0.1.0 (path+file:///Users/fakeuser/local/libra/language/stackless-bytecode/tree_heap)"),
            ("language/stdlib", "stdlib 0.1.0 (path+file:///Users/fakeuser/local/libra/language/stdlib)"),
            ("language/tools/cost-synthesis", "cost-synthesis 0.1.0 (path+file:///Users/fakeuser/local/libra/language/tools/cost-synthesis)"),
            ("language/tools/test-generation", "test-generation 0.1.0 (path+file:///Users/fakeuser/local/libra/language/tools/test-generation)"),
            ("language/transaction-builder", "transaction-builder 0.1.0 (path+file:///Users/fakeuser/local/libra/language/transaction-builder)"),
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
        ], HashMap::new())
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
        }
    }

    fn with_deps(mut self, mut deps: Vec<(&'static str, &'static str)>) -> Self {
        deps.sort();
        self.deps = Some(deps);
        self
    }

    fn with_reverse_deps(mut self, mut reverse_deps: Vec<(&'static str, &'static str)>) -> Self {
        reverse_deps.sort();
        self.reverse_deps = Some(reverse_deps);
        self
    }

    fn insert_into(self, map: &mut HashMap<PackageId, PackageDetails>) {
        map.insert(self.id.clone(), self);
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

/// Helper for creating `PackageId` instances in test code.
pub(crate) fn package_id(s: impl Into<String>) -> PackageId {
    PackageId { repr: s.into() }
}
