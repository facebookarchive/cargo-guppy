// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use guppy::{
    graph::{DependencyDirection, PackageGraph},
    PackageId,
};
use proptest::{collection::vec, prelude::*};
use proptest_ext::ValueGenerator;

pub fn construct_benchmarks(c: &mut Criterion) {
    c.bench_function("make_package_graph", |b| b.iter(|| make_package_graph()));
}

pub fn query_benchmarks(c: &mut Criterion) {
    let package_graph = make_package_graph();
    let mut cache = package_graph.new_depends_cache();
    let mut gen = ValueGenerator::deterministic();

    c.bench_function("depends_on", |b| {
        b.iter_batched_ref(
            || gen.generate(id_pairs_strategy(&package_graph)),
            |package_ids| {
                package_ids.iter().for_each(|(package_a, package_b)| {
                    let _ = package_graph.depends_on(package_a, package_b);
                })
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("depends_on_cache", |b| {
        b.iter_batched_ref(
            || gen.generate(id_pairs_strategy(&package_graph)),
            |package_ids| {
                package_ids.iter().for_each(|(package_a, package_b)| {
                    let _ = cache.depends_on(package_a, package_b);
                })
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("into_ids", |b| {
        b.iter_batched_ref(
            || gen.generate(ids_directions_strategy(&package_graph)),
            |ids_directions| {
                ids_directions
                    .iter()
                    .for_each(|(package_ids, query_direction, iter_direction)| {
                        let query = package_graph
                            .query_directed(package_ids.iter().copied(), *query_direction)
                            .unwrap();
                        let _: Vec<_> = query.resolve().package_ids(*iter_direction).collect();
                    })
            },
            BatchSize::SmallInput,
        )
    });
}

fn make_package_graph() -> PackageGraph {
    // Use this package graph as a large and representative one.
    PackageGraph::from_json(include_str!("../../../fixtures/large/metadata_libra.json")).unwrap()
}

/// Generate pairs of IDs for benchmarks.
fn id_pairs_strategy<'g>(
    graph: &'g PackageGraph,
) -> impl Strategy<Value = Vec<(&'g PackageId, &'g PackageId)>> + 'g {
    vec(
        (graph.prop010_id_strategy(), graph.prop010_id_strategy()),
        256,
    )
}

/// Generate IDs and directions for benchmarks.
fn ids_directions_strategy<'g>(
    graph: &'g PackageGraph,
) -> impl Strategy<Value = Vec<(Vec<&'g PackageId>, DependencyDirection, DependencyDirection)>> + 'g
{
    vec(
        (
            vec(graph.prop010_id_strategy(), 32),
            any::<DependencyDirection>(),
            any::<DependencyDirection>(),
        ),
        16,
    )
}

criterion_group!(benches, construct_benchmarks, query_benchmarks);
criterion_main!(benches);
