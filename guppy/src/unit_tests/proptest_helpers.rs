// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::{DependencyDirection, PackageGraph, PackageResolver, Prop09Resolver};
use crate::unit_tests::dep_helpers::{assert_link_order, GraphAssert, GraphMetadata, GraphSet};
use crate::PackageId;
use pretty_assertions::assert_eq;
use proptest::collection::vec;
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
            fn proptest_feature_select_depends_on() {
                let fixture = Fixture::$name();
                let package_graph = fixture.graph();
                let feature_graph = package_graph.feature_graph();

                proptest!(|(
                    ids in vec(feature_graph.prop09_id_strategy(), 1..16),
                    select_direction in any::<DependencyDirection>(),
                    query_direction in any::<DependencyDirection>(),
                    query_indexes in vec(any::<Index>(), 0..16),
                )| {
                    depends_on(feature_graph, &ids, select_direction, query_direction, query_indexes, "feature_select_depends_on");
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

            #[test]
            fn proptest_resolver_retain_equivalence() {
                let fixture = Fixture::$name();
                let package_graph = fixture.graph();

                // Reduce the number of tests because each run is kind of expensive.
                proptest!(ProptestConfig::with_cases(64), |(
                    ids in vec(package_graph.prop09_id_strategy(), 1..16),
                    direction in any::<DependencyDirection>(),
                    resolver in package_graph.prop09_resolver_strategy(),
                )| {
                    resolver_retain_equivalence(&mut package_graph.clone(), &ids, direction, resolver);
                });
            }

            #[test]
            fn proptest_resolve_contains() {
                let fixture = Fixture::$name();
                let package_graph = fixture.graph();

                proptest!(|(
                    select_ids in vec(package_graph.prop09_id_strategy(), 1..16),
                    direction in any::<DependencyDirection>(),
                    query_ids in vec(package_graph.prop09_id_strategy(), 0..64),
                )| {
                    resolve_contains(package_graph, &select_ids, direction, &query_ids);
                });
            }

            #[test]
            fn proptest_feature_resolve_contains() {
                let fixture = Fixture::$name();
                let package_graph = fixture.graph();
                let feature_graph = package_graph.feature_graph();

                proptest!(|(
                    select_ids in vec(feature_graph.prop09_id_strategy(), 1..16),
                    direction in any::<DependencyDirection>(),
                    query_ids in vec(feature_graph.prop09_id_strategy(), 0..64),
                )| {
                    resolve_contains(feature_graph, &select_ids, direction, &query_ids);
                });
            }

            #[test]
            fn proptest_resolve_ops() {
                let fixture = Fixture::$name();
                let package_graph = fixture.graph();

                proptest!(|(
                    resolve_tree in ResolveTree::strategy(package_graph.prop09_id_strategy())
                )| {
                    resolve_ops(package_graph, resolve_tree);
                });
            }

            #[test]
            fn proptest_feature_resolve_ops() {
                let fixture = Fixture::$name();
                let package_graph = fixture.graph();
                let feature_graph = package_graph.feature_graph();

                proptest!(|(
                    resolve_tree in ResolveTree::strategy(feature_graph.prop09_id_strategy())
                )| {
                    resolve_ops(feature_graph, resolve_tree);
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
    let reachable_ids = graph.ids(ids, select_direction, query_direction);

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
        select
            .clone()
            .resolve()
            .into_root_ids(query_direction)
            .collect()
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

/// Test that using a custom resolver and using retain_edges produce the same results.
pub(super) fn resolver_retain_equivalence(
    graph: &mut PackageGraph,
    ids: &[&PackageId],
    direction: DependencyDirection,
    resolver: Prop09Resolver,
) {
    let mut resolver_ids: Vec<_> = graph
        .select_directed(ids.iter().copied(), direction)
        .unwrap()
        .resolve_with(&resolver)
        .into_ids(direction)
        // Clone to release borrow on the graph.
        .cloned()
        .collect();
    // While we're here, might as well check topological order.
    (&*graph).assert_topo_order(&resolver_ids, direction, "topo order for resolver IDs");
    // Sort because the topological order may be different from above.
    resolver_ids.sort();

    // Now do the filtering through retain_edges.
    graph.retain_edges(|_data, link| resolver.accept(link));
    let mut retain_ids: Vec<_> = graph
        .select_directed(ids.iter().copied(), direction)
        .unwrap()
        .resolve()
        .into_ids(direction)
        // Clone because PartialEq isn't implemented for &PackageId and PackageId :/ sigh.
        .cloned()
        .collect();
    (&*graph).assert_topo_order(&retain_ids, direction, "topo order for retain IDs");
    // Sort because the topological order may be different from above.
    retain_ids.sort();

    assert_eq!(
        resolver_ids, retain_ids,
        "ids through resolver and retain_edges should be the same"
    );
}

pub(super) fn resolve_contains<'g, G: GraphAssert<'g>>(
    graph: G,
    select_ids: &[G::Id],
    direction: DependencyDirection,
    query_ids: &[G::Id],
) {
    let resolve_set = graph.resolve(select_ids, direction);
    for query_id in query_ids {
        if resolve_set.contains(*query_id) {
            graph.assert_depends_on_any(select_ids, *query_id, direction, "contains => depends on");
        } else {
            for select_id in select_ids {
                graph.assert_not_depends_on(
                    *select_id,
                    *query_id,
                    direction,
                    "not contains => not depends on",
                );
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum ResolveTree<G: GraphAssert<'static>> {
    Resolve {
        initials: Vec<G::Id>,
        direction: DependencyDirection,
    },
    Union(Box<ResolveTree<G>>, Box<ResolveTree<G>>),
    Intersection(Box<ResolveTree<G>>, Box<ResolveTree<G>>),
    Difference(Box<ResolveTree<G>>, Box<ResolveTree<G>>),
    SymmetricDifference(Box<ResolveTree<G>>, Box<ResolveTree<G>>),
}

// The 'statics are required because prop_recursive requires the leaf to be 'static.
impl<G: GraphAssert<'static> + 'static> ResolveTree<G> {
    pub(super) fn strategy(
        id_strategy: impl Strategy<Value = G::Id> + 'static,
    ) -> impl Strategy<Value = ResolveTree<G>> + 'static {
        let leaf = (vec(id_strategy, 1..16), any::<DependencyDirection>()).prop_map(
            |(initials, direction)| ResolveTree::Resolve {
                initials,
                direction,
            },
        );
        leaf.prop_recursive(
            4,  // 4 levels deep
            16, // 2**4 = 16 nodes max
            2,  // 2 items per non-leaf node,
            |inner| {
                prop_oneof![
                    (inner.clone(), inner.clone())
                        .prop_map(|(a, b)| ResolveTree::Union(Box::new(a), Box::new(b))),
                    (inner.clone(), inner.clone())
                        .prop_map(|(a, b)| ResolveTree::Intersection(Box::new(a), Box::new(b))),
                    (inner.clone(), inner.clone())
                        .prop_map(|(a, b)| ResolveTree::Difference(Box::new(a), Box::new(b))),
                    (inner.clone(), inner).prop_map(|(a, b)| ResolveTree::SymmetricDifference(
                        Box::new(a),
                        Box::new(b)
                    )),
                ]
            },
        )
    }
}

pub(super) fn resolve_ops<G: GraphAssert<'static>>(graph: G, resolve_tree: ResolveTree<G>) {
    let (resolve, hashset) = resolve_ops_impl(graph, &resolve_tree);
    assert_eq!(
        resolve.len(),
        hashset.len(),
        "resolve and hashset lengths match"
    );
    let ids: HashSet<_> = resolve
        .ids(DependencyDirection::Forward)
        .into_iter()
        .collect();
    assert_eq!(ids, hashset, "operations on resolve and hashset match");
}

fn resolve_ops_impl<G: GraphAssert<'static>>(
    graph: G,
    resolve_tree: &ResolveTree<G>,
) -> (G::Set, HashSet<G::Id>) {
    match resolve_tree {
        ResolveTree::Resolve {
            initials,
            direction,
        } => {
            let resolve_set = graph.resolve(&initials, *direction);
            let ids = resolve_set.clone().ids(*direction).into_iter().collect();
            (resolve_set, ids)
        }
        ResolveTree::Union(a, b) => {
            let (resolve_a, hashset_a) = resolve_ops_impl(graph, a);
            let (resolve_b, hashset_b) = resolve_ops_impl(graph, b);
            (
                resolve_a.union(&resolve_b),
                hashset_a.union(&hashset_b).copied().collect(),
            )
        }
        ResolveTree::Intersection(a, b) => {
            let (resolve_a, hashset_a) = resolve_ops_impl(graph, a);
            let (resolve_b, hashset_b) = resolve_ops_impl(graph, b);
            (
                resolve_a.intersection(&resolve_b),
                hashset_a.intersection(&hashset_b).copied().collect(),
            )
        }
        ResolveTree::Difference(a, b) => {
            let (resolve_a, hashset_a) = resolve_ops_impl(graph, a);
            let (resolve_b, hashset_b) = resolve_ops_impl(graph, b);
            (
                resolve_a.difference(&resolve_b),
                hashset_a.difference(&hashset_b).copied().collect(),
            )
        }
        ResolveTree::SymmetricDifference(a, b) => {
            let (resolve_a, hashset_a) = resolve_ops_impl(graph, a);
            let (resolve_b, hashset_b) = resolve_ops_impl(graph, b);
            (
                resolve_a.symmetric_difference(&resolve_b),
                hashset_a
                    .symmetric_difference(&hashset_b)
                    .copied()
                    .collect(),
            )
        }
    }
}

// TODO: Test FeatureFilter implementations.
