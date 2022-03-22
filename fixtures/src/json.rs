// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{
    details::{FixtureDetails, LinkDetails, PackageDetails, PlatformResults},
    package_id,
};
use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use guppy::{
    errors::{FeatureBuildStage, FeatureGraphWarning},
    graph::{BuildTargetId, BuildTargetKind, PackageGraph},
    platform::{EnabledTernary, Platform, TargetFeatures},
    CargoMetadata, DependencyKind,
};
use once_cell::sync::{Lazy, OnceCell};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
};

// Metadata along with interesting crate names.
pub static METADATA1_PATH: &str = "../small/metadata1.json";
pub static METADATA1_TESTCRATE: &str = "testcrate 0.1.0 (path+file:///fakepath/testcrate)";
pub static METADATA1_DATATEST: &str =
    "datatest 0.4.2 (registry+https://github.com/rust-lang/crates.io-index)";
pub static METADATA1_REGION: &str =
    "region 2.1.2 (registry+https://github.com/rust-lang/crates.io-index)";
pub static METADATA1_DTOA: &str =
    "dtoa 0.4.4 (registry+https://github.com/rust-lang/crates.io-index)";

pub static METADATA2_PATH: &str = "../small/metadata2.json";
pub static METADATA2_TESTCRATE: &str =
    "testworkspace-crate 0.1.0 (path+file:///Users/fakeuser/local/testworkspace/testcrate)";
pub static METADATA2_WALKDIR: &str =
    "walkdir 2.2.9 (path+file:///Users/fakeuser/local/testworkspace/walkdir)";
pub static METADATA2_QUOTE: &str = "quote 1.0.2 (path+file:///Users/fakeuser/local/quote)";

pub static METADATA_BUILDDEP_PATH: &str = "../small/builddep.json";

pub static METADATA_DUPS_PATH: &str = "../small/metadata_dups.json";
pub static METADATA_DUPS_TESTCRATE: &str =
    "testcrate-dups 0.1.0 (path+file:///Users/fakeuser/local/testcrates/testcrate-dups)";
pub static METADATA_DUPS_LAZY_STATIC_1: &str =
    "lazy_static 1.4.0 (registry+https://github.com/rust-lang/crates.io-index)";
pub static METADATA_DUPS_LAZY_STATIC_02: &str =
    "lazy_static 0.2.11 (registry+https://github.com/rust-lang/crates.io-index)";
pub static METADATA_DUPS_BYTES_03: &str =
    "bytes 0.3.0 (registry+https://github.com/rust-lang/crates.io-index)";
pub static METADATA_DUPS_BYTES_05: &str =
    "bytes 0.5.4 (registry+https://github.com/rust-lang/crates.io-index)";

pub static METADATA_CYCLE1_PATH: &str = "../small/metadata_cycle1.json";
pub static METADATA_CYCLE1_BASE: &str =
    "testcycles-base 0.1.0 (path+file:///Users/fakeuser/local/testcrates/testcycles/testcycles-base)";
pub static METADATA_CYCLE1_HELPER: &str =
    "testcycles-helper 0.1.0 (path+file:///Users/fakeuser/local/testcrates/testcycles/testcycles-helper)";

pub static METADATA_CYCLE2_PATH: &str = "../small/metadata_cycle2.json";
pub static METADATA_CYCLE2_UPPER_A: &str =
    "upper-a 0.1.0 (path+file:///Users/fakeuser/local/testcrates/cycle2/upper-a)";
pub static METADATA_CYCLE2_UPPER_B: &str =
    "upper-b 0.1.0 (path+file:///Users/fakeuser/local/testcrates/cycle2/upper-b)";
pub static METADATA_CYCLE2_LOWER_A: &str =
    "lower-a 0.1.0 (path+file:///Users/fakeuser/local/testcrates/cycle2/lower-a)";
pub static METADATA_CYCLE2_LOWER_B: &str =
    "lower-b 0.1.0 (path+file:///Users/fakeuser/local/testcrates/cycle2/lower-b)";

pub static METADATA_CYCLE_FEATURES_PATH: &str = "../small/metadata_cycle_features.json";
pub static METADATA_CYCLE_FEATURES_BASE: &str =
    "testcycles-base 0.1.0 (path+file:///fakepath/testcycles-features/testcycles-base)";
pub static METADATA_CYCLE_FEATURES_HELPER: &str =
    "testcycles-helper 0.1.0 (path+file:///fakepath/testcycles-features/testcycles-helper)";

pub static METADATA_TARGETS1_PATH: &str = "../small/metadata_targets1.json";
pub static METADATA_TARGETS1_TESTCRATE: &str =
    "testcrate-targets 0.1.0 (path+file:///Users/fakeuser/local/testcrates/testcrate-targets)";
pub static METADATA_TARGETS1_LAZY_STATIC_1: &str =
    "lazy_static 1.4.0 (registry+https://github.com/rust-lang/crates.io-index)";
pub static METADATA_TARGETS1_LAZY_STATIC_02: &str =
    "lazy_static 0.2.11 (registry+https://github.com/rust-lang/crates.io-index)";
pub static METADATA_TARGETS1_LAZY_STATIC_01: &str =
    "lazy_static 0.1.16 (registry+https://github.com/rust-lang/crates.io-index)";
pub static METADATA_TARGETS1_BYTES: &str =
    "bytes 0.5.3 (registry+https://github.com/rust-lang/crates.io-index)";
pub static METADATA_TARGETS1_DEP_A: &str =
    "dep-a 0.1.0 (path+file:///Users/fakeuser/local/testcrates/dep-a)";

pub static METADATA_BUILD_TARGETS1_PATH: &str = "../small/metadata_build_targets1.json";
pub static METADATA_BUILD_TARGETS1_TESTCRATE: &str =
    "testcrate 0.1.0 (path+file:///Users/fakeuser/local/testcrates/test-build-targets)";

pub static METADATA_PROC_MACRO1_PATH: &str = "../small/metadata_proc_macro1.json";
pub static METADATA_PROC_MACRO1_MACRO: &str =
    "macro 0.1.0 (path+file:///Users/fakeuser/local/testcrates/proc-macro/macro)";
pub static METADATA_PROC_MACRO1_NORMAL_USER: &str =
    "normal-user 0.1.0 (path+file:///Users/fakeuser/local/testcrates/proc-macro/normal-user)";
pub static METADATA_PROC_MACRO1_BUILD_USER: &str =
    "build-user 0.1.0 (path+file:///Users/fakeuser/local/testcrates/proc-macro/build-user)";
pub static METADATA_PROC_MACRO1_DEV_USER: &str =
    "dev-user 0.1.0 (path+file:///Users/fakeuser/local/testcrates/proc-macro/dev-user)";

pub static METADATA_ALTERNATE_REGISTRIES_PATH: &str = "../small/alternate-registries.json";
pub static METADATA_ALTERNATE_REGISTRY_URL: &str = "https://github.com/fakeorg/crates.io-index";

pub static METADATA_WEAK_NAMESPACED_FEATURES_PATH: &str = "../small/weak-namespaced-features.json";
pub static METADATA_WEAK_NAMESPACED_ID: &str =
    "namespaced-weak 0.1.0 (path+file:///home/fakeuser/dev/tmp/test-workspaces/namespaced-weak)";
pub static METADATA_WEAK_NAMESPACED_SMALLVEC: &str =
    "smallvec 1.8.0 (registry+https://github.com/rust-lang/crates.io-index)";
pub static METADATA_WEAK_NAMESPACED_ARRAYVEC: &str =
    "arrayvec 0.7.2 (registry+https://github.com/rust-lang/crates.io-index)";
pub static METADATA_WEAK_NAMESPACED_TINYVEC: &str =
    "tinyvec 1.5.1 (registry+https://github.com/rust-lang/crates.io-index)";

pub static METADATA_LIBRA_PATH: &str = "../large/metadata_libra.json";
pub static METADATA_LIBRA_ADMISSION_CONTROL_SERVICE: &str =
    "admission-control-service 0.1.0 (path+file:///Users/fakeuser/local/libra/admission_control/admission-control-service)";
pub static METADATA_LIBRA_COMPILER: &str =
    "compiler 0.1.0 (path+file:///Users/fakeuser/local/libra/language/compiler)";
pub static METADATA_LIBRA_E2E_TESTS: &str =
    "language-e2e-tests 0.1.0 (path+file:///Users/fakeuser/local/libra/language/e2e-tests)";
pub static METADATA_LIBRA_EXECUTOR: &str =
    "executor 0.1.0 (path+file:///Users/fakeuser/local/libra/execution/executor)";
pub static METADATA_LIBRA_EXECUTOR_UTILS: &str =
    "executor-utils 0.1.0 (path+file:///Users/fakeuser/local/libra/execution/executor-utils)";
pub static METADATA_LIBRA_COST_SYNTHESIS: &str =
    "cost-synthesis 0.1.0 (path+file:///Users/fakeuser/local/libra/language/tools/cost-synthesis)";
pub static METADATA_LIBRA_FUNCTIONAL_TESTS: &str =
    "functional_tests 0.1.0 (path+file:///Users/fakeuser/local/libra/language/functional_tests)";
pub static METADATA_LIBRA_FUNCTIONAL_HYPHEN_TESTS: &str =
    "functional-tests 0.1.0 (path+file:///Users/fakeuser/local/libra/language/functional-tests)";
pub static METADATA_LIBRA_LIBRA_VM: &str =
    "libra-vm 0.1.0 (path+file:///Users/fakeuser/local/libra/language/libra-vm)";
pub static METADATA_LIBRA_MOVE_LANG: &str =
    "move-lang 0.0.1 (path+file:///Users/fakeuser/local/libra/language/move-lang)";
pub static METADATA_LIBRA_MOVE_LANG_STDLIB: &str =
    "move-lang-stdlib 0.1.0 (path+file:///Users/fakeuser/local/libra/language/move-lang/stdlib)";
pub static METADATA_LIBRA_MOVE_VM_RUNTIME: &str =
    "move-vm-runtime 0.1.0 (path+file:///Users/fakeuser/local/libra/language/move-vm/runtime)";
pub static METADATA_LIBRA_STDLIB: &str =
    "stdlib 0.1.0 (path+file:///Users/fakeuser/local/libra/language/stdlib)";
pub static METADATA_LIBRA_TEST_GENERATION: &str =
    "test-generation 0.1.0 (path+file:///Users/fakeuser/local/libra/language/tools/test-generation)";
pub static METADATA_LIBRA_TRANSACTION_BUILDER: &str =
    "transaction-builder 0.1.0 (path+file:///Users/fakeuser/local/libra/language/transaction-builder)";
pub static METADATA_LIBRA_VM_GENESIS: &str =
    "vm-genesis 0.1.0 (path+file:///Users/fakeuser/local/libra/language/tools/vm-genesis)";
pub static METADATA_LIBRA_LANGUAGE_BENCHMARKS: &str =
    "language_benchmarks 0.1.0 (path+file:///Users/fakeuser/local/libra/language/benchmarks)";
pub static METADATA_LIBRA_TREE_HEAP: &str =
    "tree_heap 0.1.0 (path+file:///Users/fakeuser/local/libra/language/stackless-bytecode/tree_heap)";
pub static METADATA_LIBRA_LAZY_STATIC: &str =
    "lazy_static 1.4.0 (registry+https://github.com/rust-lang/crates.io-index)";
pub static METADATA_LIBRA_BACKTRACE: &str =
    "backtrace 0.3.37 (registry+https://github.com/rust-lang/crates.io-index)";
pub static METADATA_LIBRA_CFG_IF: &str =
    "cfg-if 0.1.9 (registry+https://github.com/rust-lang/crates.io-index)";

pub static METADATA_CARGO_NEXTEST_PATH: &str = "../small/metadata-cargo-nextest.json";

pub static METADATA_LIBRA_F0091A4_PATH: &str = "../large/metadata_libra_f0091a4.json";

pub static METADATA_LIBRA_9FFD93B_PATH: &str = "../large/metadata_libra_9ffd93b.json";

pub static METADATA_GUPPY_78CB7E8_PATH: &str = "../guppy/metadata_guppy_78cb7e8.json";

pub static METADATA_GUPPY_869476C_PATH: &str = "../guppy/metadata_guppy_869476c.json";

pub static METADATA_GUPPY_C9B4F76_PATH: &str = "../guppy/metadata_guppy_c9b4f76.json";

pub static METADATA_GUPPY_44B62FA_PATH: &str = "../guppy/metadata_guppy_44b62fa.json";
pub static METADATA_GUPPY_CARGO_GUPPY: &str =
    "cargo-guppy 0.1.0 (path+file:///home/fakeuser/dev/cargo-guppy/cargo-guppy)";

pub static FAKE_AUTHOR: &str = "Fake Author <fakeauthor@example.com>";

macro_rules! define_fixtures {
    ($($name: ident => $json_path: ident,)*) => {
        impl JsonFixture {
            // Access all fixtures.
            pub fn all_fixtures() -> &'static BTreeMap<&'static str, JsonFixture> {
                // Provide a list of all fixtures.
                static ALL_FIXTURES: Lazy<BTreeMap<&'static str, JsonFixture>> = Lazy::new(|| {
                    let mut map = BTreeMap::new();

                    $(map.insert(
                        stringify!($name),
                        JsonFixture::new(stringify!($name), $json_path, FixtureDetails::$name()),
                    );)*

                    map
                });

                &*ALL_FIXTURES
            }

            // Access individual fixtures if the name is known.
            $(pub fn $name() -> &'static Self {
                &JsonFixture::all_fixtures()[stringify!($name)]
            })*
        }
    };
}

define_fixtures! {
    metadata1 => METADATA1_PATH,
    metadata2 => METADATA2_PATH,
    metadata_builddep => METADATA_BUILDDEP_PATH,
    metadata_dups => METADATA_DUPS_PATH,
    metadata_cycle1 => METADATA_CYCLE1_PATH,
    metadata_cycle2 => METADATA_CYCLE2_PATH,
    metadata_cycle_features => METADATA_CYCLE_FEATURES_PATH,
    metadata_targets1 => METADATA_TARGETS1_PATH,
    metadata_build_targets1 => METADATA_BUILD_TARGETS1_PATH,
    metadata_proc_macro1 => METADATA_PROC_MACRO1_PATH,
    metadata_alternate_registries => METADATA_ALTERNATE_REGISTRIES_PATH,
    metadata_weak_namespaced_features => METADATA_WEAK_NAMESPACED_FEATURES_PATH,
    metadata_libra => METADATA_LIBRA_PATH,
    metadata_libra_f0091a4 => METADATA_LIBRA_F0091A4_PATH,
    metadata_libra_9ffd93b => METADATA_LIBRA_9FFD93B_PATH,
    metadata_guppy_78cb7e8 => METADATA_GUPPY_78CB7E8_PATH,
    metadata_guppy_869476c => METADATA_GUPPY_869476C_PATH,
    metadata_guppy_c9b4f76 => METADATA_GUPPY_C9B4F76_PATH,
    metadata_guppy_44b62fa => METADATA_GUPPY_44B62FA_PATH,
    metadata_cargo_nextest => METADATA_CARGO_NEXTEST_PATH,
}

pub struct JsonFixture {
    name: &'static str,
    workspace_path: Utf8PathBuf,
    abs_path: Utf8PathBuf,
    json_graph: OnceCell<(String, PackageGraph)>,
    details: FixtureDetails,
}

impl JsonFixture {
    fn new(name: &'static str, rel_path: &'static str, details: FixtureDetails) -> Self {
        let rel_path = Utf8Path::new(rel_path);
        let fixtures_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
        // rel_path is relative to this dir.
        let mut abs_path = fixtures_dir.join("src");
        abs_path.push(rel_path);
        let abs_path = Utf8PathBuf::from_path_buf(
            abs_path
                .canonicalize()
                .expect("fixture path canonicalization succeeded"),
        )
        .expect("valid UTF-8 path");

        let workspace_root = fixtures_dir.parent().expect("up to workspace root");
        let workspace_path = Utf8PathBuf::from_path_buf(
            pathdiff::diff_paths(&abs_path, workspace_root)
                .expect("both abs_path and workspace root are absolute"),
        )
        .expect("diff of UTF-8 paths is UTF-8");

        // No symlinks in this repo, so normalize this path.
        let workspace_path = normalize_assuming_no_symlinks(&workspace_path);

        Self {
            name,
            workspace_path,
            abs_path,
            json_graph: OnceCell::new(),
            details,
        }
    }

    /// Lookup a fixture by name, or `None` if the name wasn't found.
    pub fn by_name(name: &str) -> Option<&'static Self> {
        Self::all_fixtures().get(name)
    }

    /// Returns the name of this fixture.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the absolute path of this fixture.
    pub fn abs_path(&self) -> &Utf8Path {
        &self.abs_path
    }

    /// Returns the path of this fixture, relative to the workspace root.
    pub fn workspace_path(&self) -> &Utf8Path {
        &self.workspace_path
    }

    /// Returns the unparsed JSON string for this fixture.
    pub fn json(&self) -> &str {
        self.init_graph().0
    }

    /// Returns the package graph for this fixture.
    pub fn graph(&self) -> &PackageGraph {
        self.init_graph().1
    }

    /// Returns the test details for this fixture.
    pub fn details(&self) -> &FixtureDetails {
        &self.details
    }

    /// Verifies that the parsed metadata matches known details.
    pub fn verify(&self) {
        let graph = self.graph();

        graph.verify().expect("graph verification should succeed");

        // Check that all external sources parse correctly in all graphs.
        for package in graph.packages() {
            let source = package.source();
            if source.is_external() {
                let external = source
                    .parse_external()
                    .unwrap_or_else(|| panic!("cannot parse external source {}", source));
                assert_eq!(
                    format!("{}", external),
                    source.external_source().expect("is_external is true"),
                    "roundtrip with ExternalSource"
                );
            }
        }

        self.details.assert_cycles(graph, "cycles");

        self.details.assert_workspace(graph.workspace());
        self.details.assert_topo(graph);

        for id in self.details.known_ids() {
            let msg = format!("error while verifying package '{}'", id);
            let metadata = graph.metadata(id).expect(&msg);
            self.details.assert_metadata(id, metadata, &msg);

            // Check for build targets.
            if self.details.has_build_targets(id) {
                self.details.assert_build_targets(metadata, &msg);
            }

            // Check for direct dependency queries.
            if self.details.has_deps(id) {
                self.details.assert_deps(graph, id, &msg);
            }
            if self.details.has_reverse_deps(id) {
                self.details.assert_reverse_deps(graph, id, &msg);
            }

            // Check for transitive dependency queries. Use both ID based and edge-based queries.
            if self.details.has_transitive_deps(id) {
                self.details.assert_transitive_deps(
                    graph,
                    id,
                    &format!("{} (transitive deps)", msg),
                );
            }
            if self.details.has_transitive_reverse_deps(id) {
                self.details.assert_transitive_reverse_deps(
                    graph,
                    id,
                    &format!("{} (transitive reverse deps)", msg),
                );
            }

            // Check for named features.
            if self.details.has_named_features(id) {
                self.details
                    .assert_named_features(graph, id, &format!("{} (named features)", msg));
            }
        }

        self.details.assert_link_details(graph, "link details");

        // Tests for the feature graph.
        self.details
            .assert_feature_graph_warnings(graph, "feature graph warnings");
    }

    fn init_graph(&self) -> (&str, &PackageGraph) {
        let (json, package_graph) = self.json_graph.get_or_init(|| {
            let json = fs::read_to_string(&self.abs_path)
                .unwrap_or_else(|err| panic!("reading file '{}' failed: {}", self.abs_path, err));
            let graph = Self::parse_graph(&json);
            (json, graph)
        });
        (json.as_str(), package_graph)
    }

    fn parse_graph(json: &str) -> PackageGraph {
        let metadata =
            CargoMetadata::parse_json(json).expect("parsing metadata JSON should succeed");
        PackageGraph::from_metadata(metadata).expect("constructing package graph should succeed")
    }
}

// Thanks to @porglezomp on Twitter for this simple normalization method.
fn normalize_assuming_no_symlinks(p: impl AsRef<Utf8Path>) -> Utf8PathBuf {
    let mut out = Utf8PathBuf::new();
    for c in p.as_ref().components() {
        match c {
            Utf8Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

// Some clones in here make the code more uniform overall.
#[allow(clippy::redundant_clone)]
impl FixtureDetails {
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
        .with_workspace_path("")
        .with_build_targets(vec![(
            BuildTargetId::Binary("testcrate"),
            BuildTargetKind::Binary,
            "src/main.rs",
        )])
        .with_deps(vec![("datatest", METADATA1_DATATEST)])
        .with_reverse_deps(vec![])
        .insert_into(&mut details);

        #[rustfmt::skip]
        let datatest_deps =
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
            ];

        static LIB_TYPE: Lazy<Vec<String>> = Lazy::new(|| vec!["lib".into()]);

        PackageDetails::new(
            METADATA1_DATATEST,
            "datatest",
            "0.4.2",
            vec!["Ivan Dubrov <ivan@commure.com>"],
            Some("Data-driven tests in Rust\n"),
            Some("MIT/Apache-2.0"),
        )
        .with_crates_io()
        .with_build_targets(vec![
            (
                BuildTargetId::Library,
                BuildTargetKind::LibraryOrExample(&LIB_TYPE),
                "src/lib.rs",
            ),
            (
                BuildTargetId::BuildScript,
                BuildTargetKind::Binary,
                "build.rs",
            ),
            (
                BuildTargetId::Test("bench"),
                BuildTargetKind::Binary,
                "tests/bench.rs",
            ),
            (
                BuildTargetId::Test("datatest"),
                BuildTargetKind::Binary,
                "tests/datatest.rs",
            ),
            (
                BuildTargetId::Test("datatest_stable"),
                BuildTargetKind::Binary,
                "tests/datatest_stable.rs",
            ),
            (
                BuildTargetId::Test("datatest_stable_unsafe"),
                BuildTargetKind::Binary,
                "tests/datatest_stable_unsafe.rs",
            ),
            (
                BuildTargetId::Test("unicode"),
                BuildTargetKind::Binary,
                "tests/unicode.rs",
            ),
        ])
        .with_deps(datatest_deps)
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
        .with_workspace_path("testcrate")
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
        .with_workspace_path("walkdir")
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
        .with_local_path("../quote")
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

    pub(crate) fn metadata_cargo_nextest() -> Self {
        let details = HashMap::new();

        Self::new(details)
    }

    pub(crate) fn metadata_builddep() -> Self {
        let details = HashMap::new();

        Self::new(details)
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
        .with_workspace_path("")
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
        .with_workspace_path("")
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
        .with_local_path("../testcycles-helper")
        .with_deps(vec![("testcycles-base", METADATA_CYCLE1_BASE)])
        .with_transitive_deps(vec![METADATA_CYCLE1_BASE, METADATA_CYCLE1_HELPER])
        .with_transitive_reverse_deps(vec![METADATA_CYCLE1_BASE, METADATA_CYCLE1_HELPER])
        .insert_into(&mut details);

        Self::new(details)
            .with_workspace_members(vec![("", METADATA_CYCLE1_BASE)])
            .with_cycles(vec![vec![METADATA_CYCLE1_HELPER, METADATA_CYCLE1_BASE]])
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
        .with_workspace_path("upper-a")
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
        .with_workspace_path("upper-b")
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
        .with_workspace_path("lower-a")
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
        .with_workspace_path("lower-b")
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
                // upper-b dev-depends on upper-a, and upper-a normal-depends on upper-b.
                vec![METADATA_CYCLE2_UPPER_A, METADATA_CYCLE2_UPPER_B],
                // lower-b dev-depends on lower-a, and lower-a normal-depends on lower-b.
                vec![METADATA_CYCLE2_LOWER_A, METADATA_CYCLE2_LOWER_B],
            ])
    }

    pub(crate) fn metadata_cycle_features() -> Self {
        let details = HashMap::new();

        Self::new(details)
            .with_workspace_members(vec![
                ("testcycles-base", METADATA_CYCLE_FEATURES_BASE),
                ("testcycles-helper", METADATA_CYCLE_FEATURES_HELPER),
            ])
            .with_cycles(vec![vec![
                METADATA_CYCLE_FEATURES_HELPER,
                METADATA_CYCLE_FEATURES_BASE,
            ]])
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
        .with_workspace_path("")
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
            TargetFeatures::features(["sse", "sse2"].iter().copied()),
        )
        .unwrap();
        let x86_64_windows =
            Platform::new("x86_64-pc-windows-msvc", TargetFeatures::Unknown).unwrap();

        let mut link_details = HashMap::new();

        use EnabledTernary::*;

        // testcrate -> lazy_static 1.
        LinkDetails::new(
            package_id(METADATA_TARGETS1_TESTCRATE),
            package_id(METADATA_TARGETS1_LAZY_STATIC_1),
        )
        .with_platform_status(
            DependencyKind::Normal,
            x86_64_linux.clone(),
            PlatformResults::new((Enabled, Enabled), (Enabled, Enabled)),
        )
        .with_platform_status(
            DependencyKind::Normal,
            i686_windows.clone(),
            PlatformResults::new((Enabled, Enabled), (Enabled, Enabled)),
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
            PlatformResults::new((Enabled, Enabled), (Enabled, Enabled)),
        )
        .with_platform_status(
            DependencyKind::Normal,
            i686_windows.clone(),
            PlatformResults::new((Disabled, Disabled), (Disabled, Disabled)),
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
            PlatformResults::new((Disabled, Disabled), (Disabled, Disabled)),
        )
        .with_platform_status(
            DependencyKind::Development,
            i686_windows.clone(),
            PlatformResults::new((Enabled, Enabled), (Enabled, Enabled)),
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
            PlatformResults::new((Enabled, Enabled), (Disabled, Disabled))
                .with_feature_status("serde", (Enabled, Enabled))
                .with_feature_status("std", (Disabled, Disabled)),
        )
        .with_platform_status(
            DependencyKind::Normal,
            i686_windows.clone(),
            PlatformResults::new((Enabled, Enabled), (Enabled, Enabled))
                .with_feature_status("serde", (Enabled, Enabled))
                .with_feature_status("std", (Disabled, Disabled)),
        )
        .with_features(DependencyKind::Normal, vec!["serde"])
        .with_platform_status(
            DependencyKind::Build,
            x86_64_linux.clone(),
            PlatformResults::new((Disabled, Enabled), (Disabled, Disabled))
                .with_feature_status("serde", (Disabled, Disabled))
                .with_feature_status("std", (Disabled, Enabled)),
        )
        .with_platform_status(
            DependencyKind::Build,
            i686_windows.clone(),
            PlatformResults::new((Disabled, Disabled), (Disabled, Disabled))
                .with_feature_status("serde", (Disabled, Disabled))
                .with_feature_status("std", (Disabled, Disabled)),
        )
        .with_features(DependencyKind::Build, vec!["std"])
        .insert_into(&mut link_details);

        // testcrate -> dep-a.
        // As a normal dependency, this is optionally built by default, but on not-Windows or on x86
        // it is required.
        // As a dev dependency, it is present if sse2 or atomics are turned on.
        LinkDetails::new(
            package_id(METADATA_TARGETS1_TESTCRATE),
            package_id(METADATA_TARGETS1_DEP_A),
        )
        .with_platform_status(
            DependencyKind::Normal,
            x86_64_linux.clone(),
            PlatformResults::new((Enabled, Enabled), (Enabled, Enabled))
                .with_feature_status("foo", (Enabled, Enabled))
                .with_feature_status("bar", (Disabled, Disabled))
                .with_feature_status("baz", (Disabled, Disabled))
                .with_feature_status("quux", (Disabled, Disabled)),
        )
        .with_platform_status(
            DependencyKind::Normal,
            i686_windows.clone(),
            PlatformResults::new((Enabled, Enabled), (Enabled, Enabled))
                .with_feature_status("foo", (Disabled, Disabled))
                .with_feature_status("bar", (Enabled, Enabled))
                .with_feature_status("baz", (Disabled, Disabled))
                .with_feature_status("quux", (Disabled, Disabled)),
        )
        .with_platform_status(
            DependencyKind::Normal,
            x86_64_windows.clone(),
            PlatformResults::new((Disabled, Enabled), (Disabled, Enabled))
                .with_feature_status("foo", (Disabled, Disabled))
                .with_feature_status("bar", (Disabled, Disabled))
                .with_feature_status("baz", (Disabled, Disabled))
                .with_feature_status("quux", (Disabled, Disabled)),
        )
        .with_platform_status(
            DependencyKind::Development,
            x86_64_linux.clone(),
            // x86_64_linux uses TargetFeature::Unknown.
            PlatformResults::new((Enabled, Enabled), (Unknown, Unknown))
                .with_feature_status("foo", (Disabled, Disabled))
                .with_feature_status("bar", (Disabled, Disabled))
                .with_feature_status("baz", (Unknown, Unknown))
                .with_feature_status("quux", (Enabled, Enabled)),
        )
        .with_platform_status(
            DependencyKind::Development,
            i686_windows.clone(),
            // i686_windows turns on sse and sse2.
            PlatformResults::new((Enabled, Enabled), (Disabled, Disabled))
                .with_feature_status("foo", (Disabled, Disabled))
                .with_feature_status("bar", (Disabled, Disabled))
                .with_feature_status("baz", (Enabled, Enabled))
                .with_feature_status("quux", (Enabled, Enabled)),
        )
        .with_platform_status(
            DependencyKind::Development,
            x86_64_windows.clone(),
            // x86_64_windows uses TargetFeatures::Unknown.
            PlatformResults::new((Unknown, Unknown), (Disabled, Disabled))
                .with_feature_status("foo", (Disabled, Disabled))
                .with_feature_status("bar", (Disabled, Disabled))
                .with_feature_status("baz", (Unknown, Unknown))
                .with_feature_status("quux", (Unknown, Unknown)),
        )
        .with_platform_status(
            DependencyKind::Build,
            x86_64_linux.clone(),
            // x86_64_linux uses TargetFeature::Unknown.
            PlatformResults::new((Unknown, Enabled), (Disabled, Enabled))
                .with_feature_status("foo", (Unknown, Unknown))
                .with_feature_status("bar", (Disabled, Unknown))
                .with_feature_status("baz", (Disabled, Disabled))
                .with_feature_status("quux", (Disabled, Disabled)),
        )
        .with_platform_status(
            DependencyKind::Build,
            i686_windows.clone(),
            // i686_windows turns on sse and sse2.
            PlatformResults::new((Enabled, Enabled), (Disabled, Enabled))
                .with_feature_status("foo", (Enabled, Enabled))
                .with_feature_status("bar", (Disabled, Disabled))
                .with_feature_status("baz", (Disabled, Disabled))
                .with_feature_status("quux", (Disabled, Disabled)),
        )
        .with_platform_status(
            DependencyKind::Build,
            x86_64_windows.clone(),
            // x86_64_windows uses TargetFeatures::Unknown.
            PlatformResults::new((Unknown, Unknown), (Disabled, Unknown))
                .with_feature_status("foo", (Unknown, Unknown))
                .with_feature_status("bar", (Disabled, Disabled))
                .with_feature_status("baz", (Disabled, Disabled))
                .with_feature_status("quux", (Disabled, Disabled)),
        )
        .insert_into(&mut link_details);

        Self::new(details)
            .with_workspace_members(vec![("", METADATA_TARGETS1_TESTCRATE)])
            .with_link_details(link_details)
    }

    pub(crate) fn metadata_build_targets1() -> Self {
        // [package]
        // name = "testcrate"
        // version = "0.1.0"
        // authors = ["Fake Author <fakeauthor@example.com>"]
        // edition = "2018"
        // build = "build.rs"
        //
        // [lib]
        // name = "bench1"
        // crate-type = ["cdylib", "bin"]
        //
        // [[bench]]
        // name = "bench1"
        // path = "src/main.rs"
        //
        // [[bench]]
        // name = "bench2"
        // path = "src/main2.rs"
        //
        // [[example]]
        // name = "example1"
        // path = "src/lib.rs"
        // crate-type = ["rlib", "dylib"]

        let mut details = HashMap::new();

        static BIN_CDYLIB_TYPES: Lazy<Vec<String>> =
            Lazy::new(|| vec!["bin".into(), "cdylib".into()]);
        static DYLIB_RLIB_TYPES: Lazy<Vec<String>> =
            Lazy::new(|| vec!["dylib".into(), "rlib".into()]);

        PackageDetails::new(
            METADATA_BUILD_TARGETS1_TESTCRATE,
            "testcrate",
            "0.1.0",
            vec![FAKE_AUTHOR],
            None,
            None,
        )
        .with_workspace_path("")
        .with_build_targets(vec![
            (
                BuildTargetId::Library,
                BuildTargetKind::LibraryOrExample(&BIN_CDYLIB_TYPES),
                "src/lib.rs",
            ),
            (
                BuildTargetId::BuildScript,
                BuildTargetKind::Binary,
                "build.rs",
            ),
            (
                BuildTargetId::Binary("testcrate"),
                BuildTargetKind::Binary,
                "src/main.rs",
            ),
            (
                BuildTargetId::Example("example1"),
                BuildTargetKind::LibraryOrExample(&DYLIB_RLIB_TYPES),
                "src/lib.rs",
            ),
            (
                BuildTargetId::Benchmark("bench1"),
                BuildTargetKind::Binary,
                "src/main.rs",
            ),
            (
                BuildTargetId::Benchmark("bench2"),
                BuildTargetKind::Binary,
                "src/main2.rs",
            ),
        ])
        .insert_into(&mut details);

        Self::new(details)
    }

    pub(crate) fn metadata_proc_macro1() -> Self {
        let mut details = HashMap::new();

        PackageDetails::new(
            METADATA_PROC_MACRO1_MACRO,
            "macro",
            "0.1.0",
            vec![FAKE_AUTHOR],
            None,
            None,
        )
        .with_workspace_path("macro")
        .with_reverse_deps(vec![
            ("macro", METADATA_PROC_MACRO1_NORMAL_USER),
            ("macro", METADATA_PROC_MACRO1_BUILD_USER),
            ("macro", METADATA_PROC_MACRO1_DEV_USER),
        ])
        .insert_into(&mut details);

        Self::new(details)
    }

    pub(crate) fn metadata_alternate_registries() -> Self {
        let details = HashMap::new();
        Self::new(details)
    }

    pub(crate) fn metadata_weak_namespaced_features() -> Self {
        let details = HashMap::new();
        Self::new(details)
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
        .with_workspace_path("language/e2e-tests")
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
        .with_crates_io()
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
            METADATA_LIBRA_VM_GENESIS,
            METADATA_LIBRA_MOVE_LANG_STDLIB,
            METADATA_LIBRA_MOVE_LANG,
        ]])
    }

    pub(crate) fn metadata_libra_9ffd93b() -> Self {
        let details = HashMap::new();

        Self::new(details).with_cycles(vec![
            vec![METADATA_LIBRA_EXECUTOR_UTILS, METADATA_LIBRA_EXECUTOR],
            vec![
                METADATA_LIBRA_FUNCTIONAL_HYPHEN_TESTS,
                METADATA_LIBRA_E2E_TESTS,
                METADATA_LIBRA_COMPILER,
                METADATA_LIBRA_VM_GENESIS,
                METADATA_LIBRA_LIBRA_VM,
                METADATA_LIBRA_MOVE_VM_RUNTIME,
                METADATA_LIBRA_TRANSACTION_BUILDER,
                METADATA_LIBRA_STDLIB,
                METADATA_LIBRA_MOVE_LANG,
            ],
        ])
    }

    pub(crate) fn metadata_guppy_78cb7e8() -> Self {
        let details = HashMap::new();

        Self::new(details)
    }

    pub(crate) fn metadata_guppy_869476c() -> Self {
        let details = HashMap::new();

        Self::new(details)
    }

    pub(crate) fn metadata_guppy_c9b4f76() -> Self {
        let details = HashMap::new();

        Self::new(details)
    }

    pub(crate) fn metadata_guppy_44b62fa() -> Self {
        let details = HashMap::new();

        Self::new(details)
    }
}
