// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::feature::{all_filter, none_filter, FeatureId, FeatureSet};
use crate::graph::{DependencyDirection, PackageGraph, Prop09Resolver};
use crate::unit_tests::dep_helpers::{
    assert_link_order, GraphAssert, GraphMetadata, GraphQuery, GraphSet,
};
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
            use proptest::collection::{hash_set, vec};
            use proptest::prelude::*;
            use proptest::sample::Index;

            #[test]
            fn proptest_query_depends_on() {
                let fixture = Fixture::$name();
                let graph = fixture.graph();

                proptest!(|(
                    ids in vec(graph.prop09_id_strategy(), 1..16),
                    query_direction in any::<DependencyDirection>(),
                    iter_direction in any::<DependencyDirection>(),
                    query_indexes in vec(any::<Index>(), 0..16),
                )| {
                    depends_on(graph, &ids, query_direction, iter_direction, query_indexes, "query_depends_on");
                });
            }

            #[test]
            fn proptest_feature_query_depends_on() {
                let fixture = Fixture::$name();
                let package_graph = fixture.graph();
                let feature_graph = package_graph.feature_graph();

                proptest!(|(
                    ids in vec(feature_graph.prop09_id_strategy(), 1..16),
                    query_direction in any::<DependencyDirection>(),
                    iter_direction in any::<DependencyDirection>(),
                    query_indexes in vec(any::<Index>(), 0..16),
                )| {
                    depends_on(feature_graph, &ids, query_direction, iter_direction, query_indexes, "feature_query_depends_on");
                });
            }

            #[test]
            fn proptest_depends_on_same_package_id() {
                let fixture = Fixture::$name();
                let package_graph = fixture.graph();

                proptest!(|(query_id in package_graph.prop09_id_strategy())| {
                    depends_on_same_id(package_graph, query_id);
                });
            }

            #[test]
            fn proptest_depends_on_same_feature_id() {
                let fixture = Fixture::$name();
                let package_graph = fixture.graph();
                let feature_graph = package_graph.feature_graph();

                proptest!(|(query_id in feature_graph.prop09_id_strategy())| {
                    depends_on_same_id(feature_graph, query_id);
                });
            }

            #[test]
            fn proptest_query_link_order() {
                let fixture = Fixture::$name();
                let graph = fixture.graph();

                proptest!(|(
                    ids in vec(graph.prop09_id_strategy(), 1..16),
                    query_direction in any::<DependencyDirection>(),
                    iter_direction in any::<DependencyDirection>(),
                )| {
                    link_order(graph, &ids, query_direction, iter_direction, "query_link_order");
                });
            }

            #[test]
            fn proptest_query_roots() {
                let fixture = Fixture::$name();
                let graph = fixture.graph();

                proptest!(|(
                    ids in vec(graph.prop09_id_strategy(), 1..16),
                    query_direction in any::<DependencyDirection>(),
                    iter_direction in any::<DependencyDirection>(),
                    query_indexes in vec((any::<Index>(), any::<Index>()), 0..128),
                )| {
                    roots(
                        graph,
                        &ids,
                        query_direction,
                        iter_direction,
                        query_indexes,
                        "query_roots",
                    )?;
                });
            }

            #[test]
            fn proptest_feature_query_roots() {
                let fixture = Fixture::$name();
                let package_graph = fixture.graph();
                let feature_graph = package_graph.feature_graph();

                proptest!(|(
                    ids in vec(feature_graph.prop09_id_strategy(), 1..16),
                    query_direction in any::<DependencyDirection>(),
                    iter_direction in any::<DependencyDirection>(),
                    query_indexes in vec((any::<Index>(), any::<Index>()), 0..128),
                )| {
                    roots(
                        feature_graph,
                        &ids,
                        query_direction,
                        iter_direction,
                        query_indexes,
                        "feature_query_roots",
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
                    mut resolver in package_graph.prop09_resolver_strategy(),
                )| {
                    resolver.check_depends_on(true);
                    resolver_retain_equivalence(&mut package_graph.clone(), &ids, direction, resolver);
                });
            }

            #[test]
            fn proptest_resolve_contains() {
                let fixture = Fixture::$name();
                let package_graph = fixture.graph();

                proptest!(|(
                    query_ids in vec(package_graph.prop09_id_strategy(), 1..16),
                    direction in any::<DependencyDirection>(),
                    test_ids in vec(package_graph.prop09_id_strategy(), 0..64),
                )| {
                    resolve_contains(package_graph, &query_ids, direction, &test_ids);
                });
            }

            #[test]
            fn proptest_feature_resolve_contains() {
                let fixture = Fixture::$name();
                let package_graph = fixture.graph();
                let feature_graph = package_graph.feature_graph();

                proptest!(|(
                    query_ids in vec(feature_graph.prop09_id_strategy(), 1..16),
                    direction in any::<DependencyDirection>(),
                    test_ids in vec(feature_graph.prop09_id_strategy(), 0..64),
                )| {
                    resolve_contains(feature_graph, &query_ids, direction, &test_ids);
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

            #[test]
            fn proptest_package_feature_set_roundtrip() {
                let fixture = Fixture::$name();
                let package_graph = fixture.graph();
                let feature_graph = package_graph.feature_graph();

                proptest!(|(
                    query_ids in vec(package_graph.prop09_id_strategy(), 1..16),
                    query_direction in any::<DependencyDirection>(),
                    mut resolver in package_graph.prop09_resolver_strategy(),
                    test_ids in vec(feature_graph.prop09_id_strategy(), 1..16),
                    test_direction in any::<DependencyDirection>(),
                )| {
                    resolver.check_depends_on(true);
                    package_feature_set_roundtrip(package_graph, query_ids, query_direction, resolver, test_ids, test_direction);
                });
            }

            #[test]
            fn proptest_feature_set_props() {
                let fixture = Fixture::$name();
                let package_graph = fixture.graph();
                let feature_graph = package_graph.feature_graph();

                proptest!(|(
                    feature_set in feature_graph.prop09_set_strategy(),
                    direction in any::<DependencyDirection>(),
                )| {
                    feature_set_props(feature_set, direction);
                });
            }

            #[test]
            fn proptest_query_starts_from() {
                let fixture = Fixture::$name();
                let package_graph = fixture.graph();

                proptest!(|(
                    query_ids in hash_set(package_graph.prop09_id_strategy(), 0..16),
                    direction in any::<DependencyDirection>(),
                    test_ids in vec(package_graph.prop09_id_strategy(), 0..16)
                )| {
                    query_starts_from(package_graph, query_ids, direction, test_ids);
                });
            }

            #[test]
            fn proptest_feature_query_starts_from() {
                let fixture = Fixture::$name();
                let package_graph = fixture.graph();
                let feature_graph = package_graph.feature_graph();

                proptest!(|(
                    query_ids in hash_set(feature_graph.prop09_id_strategy(), 0..16),
                    direction in any::<DependencyDirection>(),
                    test_ids in vec(feature_graph.prop09_id_strategy(), 0..16)
                )| {
                    query_starts_from(feature_graph, query_ids, direction, test_ids);
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
    query_direction: DependencyDirection,
    iter_direction: DependencyDirection,
    query_indexes: Vec<Index>,
    msg: &str,
) {
    let msg = format!("{}: reachable means depends on", msg);
    let reachable_ids = graph.ids(ids, query_direction, iter_direction);

    for index in query_indexes {
        let query_id = index.get(&reachable_ids);
        graph.assert_depends_on_any(ids, *query_id, query_direction, &msg);
    }
}

/// Test depends_on and directly_depends_on semantics with the same ID.
pub(super) fn depends_on_same_id<'g, G: GraphAssert<'g>>(graph: G, query_id: G::Id) {
    graph.assert_depends_on(
        query_id,
        query_id,
        DependencyDirection::Forward,
        "depends_on for same ID returns true",
    );
    assert!(
        !graph
            .directly_depends_on(query_id, query_id)
            .expect("valid ID"),
        "directly_depends_on for same ID returns false",
    );
}

/// Test that all results of an into_iter_links query follow link order.
pub(super) fn link_order(
    graph: &PackageGraph,
    ids: &[&PackageId],
    query_direction: DependencyDirection,
    iter_direction: DependencyDirection,
    msg: &str,
) {
    let query = graph
        .query_directed(ids.iter().copied(), query_direction)
        .unwrap();
    // If the query and iter directions are the same, the set of initial IDs may be expanded
    // in case of cycles. If they are the opposite, the set of initial IDs will be different as
    // well. Compute the root IDs from the graph in that case.
    let has_cycles = graph.cycles().all_cycles().count() > 0;
    let initials = if has_cycles || query_direction != iter_direction {
        query
            .clone()
            .resolve()
            .into_root_ids(iter_direction)
            .collect()
    } else {
        ids.to_vec()
    };
    let links = query.resolve().into_links(iter_direction);
    assert_link_order(
        links,
        initials,
        iter_direction,
        &format!("{}: link order", msg),
    );
}

/// Test that the results of an `into_root_ids` query don't depend on any other root.
pub(super) fn roots<'g, G: GraphAssert<'g>>(
    graph: G,
    ids: &[G::Id],
    query_direction: DependencyDirection,
    iter_direction: DependencyDirection,
    query_indexes: Vec<(Index, Index)>,
    msg: &str,
) -> prop::test_runner::TestCaseResult {
    let root_ids = graph.root_ids(ids, query_direction, iter_direction);
    let root_id_set: HashSet<_> = root_ids.iter().copied().collect();
    assert_eq!(
        root_ids.len(),
        root_id_set.len(),
        "{}: root IDs should all be unique",
        msg
    );

    let root_metadatas = graph.root_metadatas(ids, query_direction, iter_direction);
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
            graph.assert_not_depends_on(*id1, *id2, query_direction, msg);
        }
    }
    Ok(())
}

/// Test that using a custom resolver and using retain_edges produce the same results.
pub(super) fn resolver_retain_equivalence(
    graph: &mut PackageGraph,
    ids: &[&PackageId],
    direction: DependencyDirection,
    mut resolver: Prop09Resolver,
) {
    let mut resolver_ids: Vec<_> = graph
        .query_directed(ids.iter().copied(), direction)
        .unwrap()
        .resolve_with(&mut resolver)
        .package_ids(direction)
        // Clone to release borrow on the graph.
        .cloned()
        .collect();
    // While we're here, might as well check topological order.
    (&*graph).assert_topo_order(&resolver_ids, direction, "topo order for resolver IDs");
    // Sort because the topological order may be different from above.
    resolver_ids.sort();

    // Now do the filtering through retain_edges.
    graph.retain_edges(|_data, link| resolver.accept_link(link));
    let mut retain_ids: Vec<_> = graph
        .query_directed(ids.iter().copied(), direction)
        .unwrap()
        .resolve()
        .package_ids(direction)
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
    query_ids: &[G::Id],
    direction: DependencyDirection,
    test_ids: &[G::Id],
) {
    let resolve_set = graph.resolve(query_ids, direction);
    for test_id in test_ids {
        if resolve_set.contains(*test_id) {
            graph.assert_depends_on_any(query_ids, *test_id, direction, "contains => depends on");
        } else {
            for query_id in query_ids {
                graph.assert_not_depends_on(
                    *query_id,
                    *test_id,
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
            let ids = resolve_set.ids(*direction).into_iter().collect();
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

pub(super) fn package_feature_set_roundtrip(
    package_graph: &PackageGraph,
    query_ids: Vec<&PackageId>,
    query_direction: DependencyDirection,
    mut resolver: Prop09Resolver,
    test_ids: Vec<FeatureId>,
    test_direction: DependencyDirection,
) {
    let package_set = package_graph
        .query_directed(query_ids.iter().copied(), query_direction)
        .expect("valid package IDs")
        .resolve_with(&mut resolver);
    let feature_graph = package_graph.feature_graph();
    let all_feature_set = feature_graph.resolve_packages(&package_set, all_filter());
    let no_feature_set = feature_graph.resolve_packages(&package_set, none_filter());

    for test_id in test_ids {
        assert_eq!(
            package_set
                .contains(test_id.package_id())
                .expect("valid package ID"),
            all_feature_set.contains(test_id).expect("valid feature ID"),
            "all_filter => package ID present == feature ID present"
        );

        assert_eq!(
            package_set
                .contains(test_id.package_id())
                .expect("valid package ID"),
            no_feature_set
                .contains((test_id.package_id(), None))
                .expect("valid feature ID"),
            "none_filter => package ID present == base feature ID present"
        );
    }

    let package_ids: Vec<_> = package_set.package_ids(test_direction).collect();
    let package_set_2 = all_feature_set.to_package_set();
    let package_ids_2: Vec<_> = package_set_2.package_ids(test_direction).collect();
    assert_eq!(package_ids, package_ids_2, "package IDs roundtrip");
}

pub(super) fn feature_set_props(feature_set: FeatureSet<'_>, direction: DependencyDirection) {
    // into_ids and into_packages_with_features match (after sorting).
    let mut feature_ids: Vec<_> = feature_set.feature_ids(direction).collect();
    let mut feature_ids_2: Vec<_> = feature_set
        .clone()
        .into_packages_with_features(direction)
        .flat_map(|(metadata, features): (_, Vec<_>)| {
            let package_id = metadata.id();
            features
                .into_iter()
                .map(move |feature| FeatureId::from((package_id, feature)))
        })
        .collect();
    feature_ids.sort();
    feature_ids_2.sort();

    assert_eq!(
        feature_ids, feature_ids_2,
        "into_ids and into_packages_with_features match"
    );

    // to_package_set and into_packages_with_features match (without sorting).
    let package_set_ids: Vec<_> = feature_set
        .to_package_set()
        .package_ids(direction)
        .collect();
    let feature_set_ids: Vec<_> = feature_set
        .into_packages_with_features(direction)
        .map(|(metadata, features): (_, Vec<_>)| {
            println!("for id {}, features: {:?}", metadata.id(), features);
            metadata.id()
        })
        .collect();
    assert_eq!(
        package_set_ids, feature_set_ids,
        "to_package_set and into_packages_with_features match"
    );
}

pub(super) fn query_starts_from<'g, G: GraphAssert<'g>>(
    graph: G,
    query_ids: HashSet<G::Id>,
    direction: DependencyDirection,
    test_ids: Vec<G::Id>,
) {
    let query = graph.query(query_ids.iter().copied(), direction);
    assert_eq!(query.direction(), direction, "query direction");

    for query_id in &query_ids {
        assert!(query.starts_from(*query_id), "starts from");
    }

    for test_id in test_ids {
        if !query_ids.contains(&test_id) {
            assert!(!query.starts_from(test_id), "does not start from");
        }
    }
}

// TODO: More tests for FeatureFilter implementations.
