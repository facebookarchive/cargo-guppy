// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::fixtures::{self, Fixture};
use crate::graph::feature::{all_filter, none_filter, FeatureId};
use crate::graph::{
    BuildTargetId, BuildTargetKind, DependencyDirection, DotWrite, PackageDotVisitor, PackageLink,
    PackageMetadata,
};
use std::fmt;
use std::iter;

mod small {
    use super::*;
    use crate::graph::feature::{default_filter, feature_filter};
    use crate::unit_tests::feature_helpers::assert_features_for_package;
    use crate::unit_tests::fixtures::{package_id, METADATA_PROC_MACRO1_MACRO};
    use pretty_assertions::assert_eq;

    // Test specific details extracted from metadata1.json.
    #[test]
    fn metadata1() {
        let metadata1 = Fixture::metadata1();
        metadata1.verify();

        let graph = metadata1.graph();
        let mut root_deps: Vec<_> = graph
            .dep_links(&fixtures::package_id(fixtures::METADATA1_TESTCRATE))
            .expect("root crate deps should exist")
            .collect();

        assert_eq!(root_deps.len(), 1, "the root crate has one dependency");
        let dep = root_deps.pop().expect("the root crate has one dependency");
        // XXX test for details of dependency edges as well?
        assert!(
            dep.edge.normal().is_present(),
            "normal dependency is defined"
        );
        assert!(dep.edge.build().is_present(), "build dependency is defined");
        assert!(dep.edge.dev().is_present(), "dev dependency is defined");

        // Print out dot graphs for small subgraphs.
        static EXPECTED_DOT: &str = r#"digraph {
    0 [label="winapi-x86_64-pc-windows-gnu"]
    11 [label="mach"]
    13 [label="winapi"]
    14 [label="libc"]
    20 [label="winapi-i686-pc-windows-gnu"]
    26 [label="region"]
    31 [label="bitflags"]
    11 -> 14 [label="libc"]
    13 -> 20 [label="winapi-i686-pc-windows-gnu"]
    13 -> 0 [label="winapi-x86_64-pc-windows-gnu"]
    26 -> 31 [label="bitflags"]
    26 -> 14 [label="libc"]
    26 -> 11 [label="mach"]
    26 -> 13 [label="winapi"]
}
"#;
        let actual_dot = graph
            .query_forward(iter::once(&fixtures::package_id(
                fixtures::METADATA1_REGION,
            )))
            .unwrap()
            .resolve()
            .into_dot(NameVisitor);
        assert_eq!(
            EXPECTED_DOT,
            format!("{}", actual_dot),
            "dot output matches"
        );

        // For reverse reachable ensure that the arrows are in the correct direction.
        static EXPECTED_DOT_REVERSED: &str = r#"digraph {
    1 [label="datatest"]
    9 [label="serde_yaml"]
    15 [label="dtoa"]
    18 [label="testcrate"]
    1 -> 9 [label="serde_yaml"]
    9 -> 15 [label="dtoa"]
    18 -> 1 [label="datatest"]
}
"#;
        let actual_dot_reversed = graph
            .query_reverse(iter::once(&fixtures::package_id(fixtures::METADATA1_DTOA)))
            .unwrap()
            .resolve()
            .into_dot(NameVisitor);

        assert_eq!(
            EXPECTED_DOT_REVERSED,
            format!("{}", actual_dot_reversed),
            "reversed dot output matches"
        );

        // ---

        // Check that resolve_with works by dropping all edges into libc (compare to example above).
        static EXPECTED_DOT_NO_LIBC: &str = r#"digraph {
    0 [label="winapi-x86_64-pc-windows-gnu"]
    11 [label="mach"]
    13 [label="winapi"]
    20 [label="winapi-i686-pc-windows-gnu"]
    26 [label="region"]
    31 [label="bitflags"]
    13 -> 20 [label="winapi-i686-pc-windows-gnu"]
    13 -> 0 [label="winapi-x86_64-pc-windows-gnu"]
    26 -> 31 [label="bitflags"]
    26 -> 11 [label="mach"]
    26 -> 13 [label="winapi"]
}
"#;
        let actual_dot = graph
            .query_forward(iter::once(&fixtures::package_id(
                fixtures::METADATA1_REGION,
            )))
            .unwrap()
            .resolve_with_fn(|_, link| link.to.name() != "libc")
            .into_dot(NameVisitor);
        assert_eq!(
            EXPECTED_DOT_NO_LIBC,
            format!("{}", actual_dot),
            "dot output matches"
        );

        // ---

        let feature_graph = graph.feature_graph();
        assert_eq!(feature_graph.feature_count(), 492, "feature count");
        assert_eq!(feature_graph.link_count(), 608, "link count");
        let feature_set = feature_graph.query_workspace(all_filter()).resolve();
        let root_ids: Vec<_> = feature_set
            .into_root_ids(DependencyDirection::Forward)
            .collect();
        let testcrate_id = fixtures::package_id(fixtures::METADATA1_TESTCRATE);
        let expected = vec![FeatureId::new(&testcrate_id, "datatest")];
        assert_eq!(root_ids, expected, "feature graph root IDs match");
    }

    proptest_suite!(metadata1);

    #[test]
    fn metadata2() {
        let metadata2 = Fixture::metadata2();
        metadata2.verify();

        let feature_graph = metadata2.graph().feature_graph();
        assert_eq!(feature_graph.feature_count(), 472, "feature count");
        assert_eq!(feature_graph.link_count(), 570, "link count");
        let root_ids: Vec<_> = feature_graph
            .query_workspace(none_filter())
            .resolve()
            .into_root_ids(DependencyDirection::Forward)
            .collect();
        let testcrate_id = fixtures::package_id(fixtures::METADATA2_TESTCRATE);
        let expected = vec![FeatureId::base(&testcrate_id)];
        assert_eq!(root_ids, expected, "feature graph root IDs match");
    }

    proptest_suite!(metadata2);

    #[test]
    fn metadata_dups() {
        let metadata_dups = Fixture::metadata_dups();
        metadata_dups.verify();
    }

    proptest_suite!(metadata_dups);

    #[test]
    fn metadata_cycle1() {
        let metadata_cycle1 = Fixture::metadata_cycle1();
        metadata_cycle1.verify();
    }

    proptest_suite!(metadata_cycle1);

    #[test]
    fn metadata_cycle2() {
        let metadata_cycle2 = Fixture::metadata_cycle2();
        metadata_cycle2.verify();
    }

    proptest_suite!(metadata_cycle2);

    #[test]
    fn metadata_targets1() {
        let metadata_targets1 = Fixture::metadata_targets1();
        metadata_targets1.verify();

        let package_graph = metadata_targets1.graph();
        let package_set = package_graph.resolve_all();
        let feature_graph = metadata_targets1.graph().feature_graph();
        assert_eq!(feature_graph.feature_count(), 31, "feature count");

        // Some code that might be useful for debugging.
        if false {
            for (source, target, edge) in feature_graph
                .resolve_all()
                .into_links(DependencyDirection::Forward)
            {
                let source_metadata = package_graph.metadata(source.package_id()).unwrap();
                let target_metadata = package_graph.metadata(target.package_id()).unwrap();

                println!(
                    "feature link: {}:{} {} -> {}:{} {} {:?}",
                    source_metadata.name(),
                    source_metadata.version(),
                    source.feature().unwrap_or("[base]"),
                    target_metadata.name(),
                    target_metadata.version(),
                    target.feature().unwrap_or("[base]"),
                    edge
                );
            }
        }

        assert_eq!(feature_graph.link_count(), 48, "feature link count");

        // Check that resolve_packages + a feature filter works.
        let feature_set = feature_graph.resolve_packages(
            &package_set,
            feature_filter(default_filter(), ["foo", "bar"].iter().copied()),
        );
        let dep_a_id = fixtures::package_id(fixtures::METADATA_TARGETS1_DEP_A);
        assert!(feature_set
            .contains((&dep_a_id, "foo"))
            .expect("valid feature ID"));
        assert!(feature_set
            .contains((&dep_a_id, "bar"))
            .expect("valid feature ID"));
        assert!(!feature_set
            .contains((&dep_a_id, "baz"))
            .expect("valid feature ID"));
        assert!(!feature_set
            .contains((&dep_a_id, "quux"))
            .expect("valid feature ID"));

        assert_features_for_package(
            &feature_set,
            &fixtures::package_id(fixtures::METADATA_TARGETS1_TESTCRATE),
            &[None],
            "testcrate",
        );
        assert_features_for_package(
            &feature_set,
            &dep_a_id,
            &[None, Some("foo"), Some("bar")],
            "dep a",
        );
        assert_features_for_package(
            &feature_set,
            &fixtures::package_id(fixtures::METADATA_TARGETS1_LAZY_STATIC_1),
            &[None],
            "lazy_static",
        );
    }

    proptest_suite!(metadata_targets1);

    #[test]
    fn metadata_build_targets1() {
        let metadata_build_targets1 = Fixture::metadata_build_targets1();
        metadata_build_targets1.verify();
    }

    // No need for proptests because there are no dependencies involved.

    #[test]
    fn metadata_proc_macro1() {
        let metadata = Fixture::metadata_proc_macro1();
        metadata.verify();
        let graph = metadata.graph();

        let package = graph
            .metadata(&package_id(METADATA_PROC_MACRO1_MACRO))
            .expect("valid package ID");
        assert!(package.is_proc_macro(), "is proc macro");
        assert!(matches!(
            package
                .build_target(&BuildTargetId::Library)
                .expect("library package is present")
                .kind(),
            BuildTargetKind::ProcMacro
        ));
    }

    // No need for proptests because this is a really simple test.
}

mod large {
    use super::*;
    use crate::unit_tests::dep_helpers::GraphAssert;
    use crate::unit_tests::fixtures::{
        package_id, METADATA_LIBRA_ADMISSION_CONTROL_SERVICE, METADATA_LIBRA_EXECUTOR_UTILS,
    };

    #[test]
    fn metadata_libra() {
        let metadata_libra = Fixture::metadata_libra();
        metadata_libra.verify();
    }

    proptest_suite!(metadata_libra);

    #[test]
    fn metadata_libra_f0091a4() {
        let metadata = Fixture::metadata_libra_f0091a4();
        metadata.verify();
    }

    proptest_suite!(metadata_libra_f0091a4);

    #[test]
    fn metadata_libra_9ffd93b() {
        let metadata = Fixture::metadata_libra_9ffd93b();
        metadata.verify();

        let graph = metadata.graph();
        graph.assert_depends_on(
            &package_id(METADATA_LIBRA_ADMISSION_CONTROL_SERVICE),
            &package_id(METADATA_LIBRA_EXECUTOR_UTILS),
            DependencyDirection::Forward,
            "admission-control-service should depend on executor-utils",
        );
        graph.assert_not_depends_on(
            &package_id(METADATA_LIBRA_EXECUTOR_UTILS),
            &package_id(METADATA_LIBRA_ADMISSION_CONTROL_SERVICE),
            DependencyDirection::Forward,
            "executor-utils should not depend on admission-control-service",
        );
    }

    proptest_suite!(metadata_libra_9ffd93b);
}

struct NameVisitor;

impl PackageDotVisitor for NameVisitor {
    fn visit_package(&self, package: &PackageMetadata, f: &mut DotWrite<'_, '_>) -> fmt::Result {
        write!(f, "{}", package.name())
    }

    fn visit_link(&self, link: PackageLink<'_>, f: &mut DotWrite<'_, '_>) -> fmt::Result {
        write!(f, "{}", link.edge.dep_name())
    }
}
