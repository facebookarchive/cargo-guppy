// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::{DependencyDirection, PackageGraph};
use crate::unit_tests::dep_helpers::{assert_link_order, GraphAssert, GraphMetadata};
use crate::PackageId;
use pretty_assertions::assert_eq;
use proptest::prelude::*;
use proptest::sample::Index;
use std::collections::HashSet;

macro_rules! proptest_suite {
    ($name: ident) => {
        mod $name {
            use crate::graph::DependencyDirection;
            use crate::unit_tests::fixtures::Fixture;
            use crate::unit_tests::proptest_helpers::*;
            use proptest::collection::vec;
            use proptest::prelude::*;
            use proptest::sample::Index;

            #[test]
            fn proptest_select_depends_on() {
                let fixture = Fixture::$name();
                let graph = fixture.graph();

                proptest!(|(
                    ids in vec(graph.prop09_id_strategy(), 1..16),
                    select_direction in any::<DependencyDirection>(),
                    query_direction in any::<DependencyDirection>(),
                    query_indexes in vec(any::<Index>(), 0..16),
                )| {
                    depends_on(graph, &ids, select_direction, query_direction, query_indexes, "select_depends_on");
                });
            }

            #[test]
            fn proptest_select_link_order() {
                let fixture = Fixture::$name();
                let graph = fixture.graph();

                proptest!(|(
                    ids in vec(graph.prop09_id_strategy(), 1..16),
                    select_direction in any::<DependencyDirection>(),
                    query_direction in any::<DependencyDirection>(),
                )| {
                    link_order(graph, &ids, select_direction, query_direction, "select_link_order");
                });
            }

            #[test]
            fn proptest_select_roots() {
                let fixture = Fixture::$name();
                let graph = fixture.graph();

                proptest!(|(
                    ids in vec(graph.prop09_id_strategy(), 1..16),
                    select_direction in any::<DependencyDirection>(),
                    query_direction in any::<DependencyDirection>(),
                    query_indexes in vec((any::<Index>(), any::<Index>()), 0..128),
                )| {
                    roots(
                        graph,
                        &ids,
                        select_direction,
                        query_direction,
                        query_indexes,
                        "select_roots",
                    )?;
                });
            }

            #[test]
            fn proptest_feature_select_roots() {
                let fixture = Fixture::$name();
                let package_graph = fixture.graph();
                let feature_graph = package_graph.feature_graph();

                proptest!(|(
                    ids in vec(feature_graph.prop09_id_strategy(), 1..16),
                    select_direction in any::<DependencyDirection>(),
                    query_direction in any::<DependencyDirection>(),
                    query_indexes in vec((any::<Index>(), any::<Index>()), 0..128),
                )| {
                    roots(
                        feature_graph,
                        &ids,
                        select_direction,
                        query_direction,
                        query_indexes,
                        "feature_select_roots",
                    )?;
                });
            }
        }
    }
}

/// Test that all results of an into_iter_ids query depend on at least one of the ids in the query
/// set.
pub(super) fn depends_on<'g, G: GraphAssert<'g>>(
    graph: G,
    ids: &[G::Id],
    select_direction: DependencyDirection,
    query_direction: DependencyDirection,
    query_indexes: Vec<Index>,
    msg: &str,
) {
    let msg = format!("{}: reachable means depends on", msg);
    let reachable_ids = graph.iter_ids(ids, select_direction, query_direction);

    for index in query_indexes {
        let query_id = index.get(&reachable_ids);
        graph.assert_depends_on_any(ids, *query_id, select_direction, &msg);
    }
}

/// Test that all results of an into_iter_links query follow link order.
pub(super) fn link_order(
    graph: &PackageGraph,
    ids: &[&PackageId],
    select_direction: DependencyDirection,
    query_direction: DependencyDirection,
    msg: &str,
) {
    let select = graph
        .select_directed(ids.iter().copied(), select_direction)
        .unwrap();
    // If the select and query directions are the opposite, the set of initial IDs will be
    // different as well. Compute the root IDs from the graph in that case.
    let initials = if select_direction != query_direction {
        select.clone().into_root_ids(query_direction).collect()
    } else {
        ids.to_vec()
    };
    let links = select.into_iter_links(Some(query_direction));
    assert_link_order(
        links,
        initials,
        query_direction,
        &format!("{}: link order", msg),
    );
}

/// Test that the results of an `into_root_ids` query don't depend on any other root.
pub(super) fn roots<'g, G: GraphAssert<'g>>(
    graph: G,
    ids: &[G::Id],
    select_direction: DependencyDirection,
    query_direction: DependencyDirection,
    query_indexes: Vec<(Index, Index)>,
    msg: &str,
) -> prop::test_runner::TestCaseResult {
    let root_ids = graph.root_ids(ids, select_direction, query_direction);
    let root_id_set: HashSet<_> = root_ids.iter().copied().collect();
    assert_eq!(
        root_ids.len(),
        root_id_set.len(),
        "{}: root IDs should all be unique",
        msg
    );

    let root_metadatas = graph.root_metadatas(ids, select_direction, query_direction);
    assert_eq!(
        root_ids.len(),
        root_metadatas.len(),
        "{}: root IDs and metadatas should have the same count",
        msg
    );
    let root_id_set_2: HashSet<_> = root_metadatas
        .iter()
        .map(|metadata| metadata.id())
        .collect();
    assert_eq!(
        root_id_set, root_id_set_2,
        "{}: root IDs and metadatas should return the same results",
        msg
    );

    assert!(
        !root_ids.is_empty(),
        "ids is non-empty so root ids can't be empty either"
    );
    for (index1, index2) in query_indexes {
        let id1 = index1.get(&root_ids);
        let id2 = index2.get(&root_ids);
        if id1 != id2 {
            graph.assert_not_depends_on(*id1, *id2, select_direction, msg);
        }
    }
    Ok(())
}

// TODO: Test FeatureFilter implementations.
