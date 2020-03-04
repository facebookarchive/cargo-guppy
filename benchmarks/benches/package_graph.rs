// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use guppy::graph::PackageGraph;
use guppy::PackageId;
use guppy_benchmarks::ValueGenerator;
use proptest::collection::vec;
use proptest::prelude::*;

pub fn benchmarks(c: &mut Criterion) {
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
}

fn make_package_graph() -> PackageGraph {
    // Use this package graph as a large and representative one.
    PackageGraph::from_json(include_str!("../../guppy/fixtures/large/metadata_libra.json")).unwrap()
}

/// Generate pairs of IDs for benchmarks.
fn id_pairs_strategy<'g>(
    graph: &'g PackageGraph,
) -> impl Strategy<Value = Vec<(&'g PackageId, &'g PackageId)>> + 'g {
    vec(
        (graph.prop09_id_strategy(), graph.prop09_id_strategy()),
        256,
    )
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
