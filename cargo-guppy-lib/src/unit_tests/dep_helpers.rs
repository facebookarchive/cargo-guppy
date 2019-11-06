use crate::graph::{
    DependencyDirection, DependencyLink, DependsCache, PackageGraph, PackageMetadata,
};
use crate::unit_tests::fixtures::PackageDetails;
use cargo_metadata::PackageId;
use std::collections::{BTreeSet, HashSet};
use std::iter;

fn __from_metadata<'a>(dep: &DependencyLink<'a>) -> &'a PackageMetadata {
    dep.from
}
fn __to_metadata<'a>(dep: &DependencyLink<'a>) -> &'a PackageMetadata {
    dep.to
}
type DepToMetadata<'a> = fn(&DependencyLink<'a>) -> &'a PackageMetadata;

/// Some of the messages are different based on whether we're testing forward deps or reverse
/// ones. For forward deps, we use the terms "known" for 'from' and "variable" for 'to'. For
/// reverse deps it's the other way round.
struct DirectionDesc<'a> {
    direction_desc: &'static str,
    known_desc: &'static str,
    variable_desc: &'static str,
    known_metadata: DepToMetadata<'a>,
    variable_metadata: DepToMetadata<'a>,
}

impl<'a> DirectionDesc<'a> {
    fn new(direction: DependencyDirection) -> Self {
        match direction {
            DependencyDirection::Forward => Self::forward(),
            DependencyDirection::Reverse => Self::reverse(),
        }
    }

    fn forward() -> Self {
        Self {
            direction_desc: "forward",
            known_desc: "from",
            variable_desc: "to",
            known_metadata: __from_metadata as DepToMetadata<'a>,
            variable_metadata: __to_metadata as DepToMetadata<'a>,
        }
    }

    fn reverse() -> Self {
        Self {
            direction_desc: "reverse",
            known_desc: "to",
            variable_desc: "from",
            known_metadata: __to_metadata as DepToMetadata<'a>,
            variable_metadata: __from_metadata as DepToMetadata<'a>,
        }
    }

    fn known_metadata(&self, dep: &DependencyLink<'a>) -> &'a PackageMetadata {
        (self.known_metadata)(dep)
    }

    fn variable_metadata(&self, dep: &DependencyLink<'a>) -> &'a PackageMetadata {
        (self.variable_metadata)(dep)
    }
}

pub(crate) fn assert_deps_internal(
    graph: &PackageGraph,
    direction: DependencyDirection,
    known_details: &PackageDetails,
    msg: &str,
) {
    let desc = DirectionDesc::new(direction);

    // Compare (dep_name, resolved_name, id) triples.
    let expected_dep_ids: Vec<_> = known_details
        .deps(direction)
        .unwrap_or_else(|| {
            panic!(
                "{}: {} dependencies must be present",
                msg, desc.direction_desc
            )
        })
        .iter()
        .map(|(dep_name, id)| (*dep_name, dep_name.replace("-", "_"), id))
        .collect();
    let actual_deps: Vec<_> = graph
        .dep_links_directed(known_details.id(), direction)
        .unwrap_or_else(|| panic!("{}: deps for package not found", msg))
        .into_iter()
        .collect();
    let mut actual_dep_ids: Vec<_> = actual_deps
        .iter()
        .map(|dep| {
            (
                dep.edge.dep_name(),
                dep.edge.resolved_name().to_string(),
                desc.variable_metadata(&dep).id(),
            )
        })
        .collect();
    actual_dep_ids.sort();
    assert_eq!(
        expected_dep_ids, actual_dep_ids,
        "{}: expected {} dependencies",
        msg, desc.direction_desc,
    );

    let mut cache = graph.new_depends_cache();
    for (_, _, dep_id) in &actual_dep_ids {
        // depends_on should agree with the dependencies returned.
        assert_depends_on(known_details.id(), dep_id, &mut cache, direction, msg);
    }

    // Check that the dependency metadata returned is consistent with what we expect.
    let known_msg = format!(
        "{}: {} dependency edge {} this package",
        msg, desc.direction_desc, desc.known_desc
    );
    for actual_dep in &actual_deps {
        known_details.assert_metadata(desc.known_metadata(&actual_dep), &known_msg);
        // XXX maybe compare version requirements?
    }
}

pub(crate) fn assert_transitive_deps_internal(
    graph: &PackageGraph,
    direction: DependencyDirection,
    known_details: &PackageDetails,
    msg: &str,
) {
    let desc = DirectionDesc::new(direction);

    let expected_dep_ids = known_details.transitive_deps(direction).unwrap_or_else(|| {
        panic!(
            "{}: {} transitive dependencies must be present",
            msg, desc.direction_desc
        )
    });
    // There's no impl of Eq<&PackageId> for PackageId :(
    let expected_dep_id_refs: Vec<_> = expected_dep_ids.iter().collect();

    let mut actual_dep_ids: Vec<_> = graph
        .transitive_dep_ids_directed(iter::once(known_details.id()), direction)
        .unwrap_or_else(|err| {
            panic!(
                "{}: {} transitive dep id query failed: {}",
                msg, desc.direction_desc, err
            )
        })
        .collect();
    actual_dep_ids.sort();
    let actual_deps: Vec<_> = graph
        .transitive_dep_links_directed(iter::once(known_details.id()), direction)
        .unwrap_or_else(|err| {
            panic!(
                "{}: {} transitive dep query failed: {}",
                msg, desc.direction_desc, err
            )
        })
        .collect();
    // Use a BTreeSet for unique identifiers. This is also used later for set operations.
    let ids_from_links_set: BTreeSet<_> = actual_deps
        .iter()
        .flat_map(|dep| vec![dep.from.id(), dep.to.id()])
        .collect();
    let ids_from_links: Vec<_> = ids_from_links_set.iter().copied().collect();

    assert_eq!(
        expected_dep_id_refs, actual_dep_ids,
        "{}: expected {} transitive dependency IDs",
        msg, desc.direction_desc
    );
    assert_eq!(
        expected_dep_id_refs, ids_from_links,
        "{}: expected {} transitive dependency infos",
        msg, desc.direction_desc
    );

    // The order requirements are weaker than topological -- for forward queries, a dep should show
    // up at least once in 'to' before it ever shows up in 'from'.
    assert_link_order(actual_deps, iter::once(known_details.id()), &desc, msg);

    let mut cache = graph.new_depends_cache();
    for dep_id in expected_dep_id_refs {
        // depends_on should agree with this.
        assert_depends_on(known_details.id(), dep_id, &mut cache, direction, msg);

        // Transitive deps should be transitively closed.
        let dep_actual_dep_ids: BTreeSet<_> = graph
            .transitive_dep_ids_directed(iter::once(dep_id), direction)
            .unwrap_or_else(|err| {
                panic!(
                    "{}: {} transitive dep id query failed for dependency '{}': {}",
                    msg, desc.direction_desc, dep_id.repr, err
                )
            })
            .collect();
        // Use difference instead of is_subset/is_superset for better error messages.
        let difference: Vec<_> = dep_actual_dep_ids.difference(&ids_from_links_set).collect();
        assert!(
            difference.is_empty(),
            "{}: unexpected extra {} transitive dependency IDs for dep '{}': {:?}",
            msg,
            desc.direction_desc,
            dep_id.repr,
            difference
        );

        let dep_ids_from_links: BTreeSet<_> = graph
            .transitive_dep_links_directed(iter::once(dep_id), direction)
            .unwrap_or_else(|err| {
                panic!(
                    "{}: {} transitive dep query failed for dependency '{}': {}",
                    msg, desc.direction_desc, dep_id.repr, err
                )
            })
            .flat_map(|dep| vec![dep.from.id(), dep.to.id()])
            .collect();
        // Use difference instead of is_subset/is_superset for better error messages.
        let difference: Vec<_> = dep_ids_from_links.difference(&ids_from_links_set).collect();
        assert!(
            difference.is_empty(),
            "{}: unexpected extra {} transitive dependencies for dep '{}': {:?}",
            msg,
            desc.direction_desc,
            dep_id.repr,
            difference
        );
    }
}

pub(crate) fn assert_topo_ids(graph: &PackageGraph, direction: DependencyDirection, msg: &str) {
    let topo_ids: Vec<_> = graph.topo_ids_directed(direction).collect();
    assert_eq!(
        topo_ids.len(),
        graph.package_count(),
        "{}: topo sort should return all packages",
        msg
    );

    // A package that comes later cannot depend on one that comes earlier.
    let mut cache = graph.new_depends_cache();
    for (idx, earlier_package) in topo_ids.iter().enumerate() {
        // Note that this skips over idx + 1 entries to avoid earlier_package == later_package.
        // Doing an exhaustive search would be O(n**2) in the number of packages, so just do a
        // maximum of 20.
        // TODO: use proptest to generate random queries on the corpus.
        for later_package in topo_ids.iter().skip(idx + 1).take(20) {
            assert_not_depends_on(later_package, earlier_package, &mut cache, direction, msg);
        }
    }
}

pub(crate) fn assert_all_links(graph: &PackageGraph, direction: DependencyDirection, msg: &str) {
    let desc = DirectionDesc::new(direction);
    let all_links: Vec<_> = graph.all_links_directed(direction).collect();
    assert_eq!(
        all_links.len(),
        graph.link_count(),
        "{}: all links should be returned",
        msg
    );

    // all_links should be in the correct order.
    assert_link_order(all_links, graph.root_ids_directed(direction), &desc, msg);
}

fn assert_depends_on(
    package_a: &PackageId,
    package_b: &PackageId,
    cache: &mut DependsCache,
    direction: DependencyDirection,
    msg: &str,
) {
    match direction {
        DependencyDirection::Forward => assert!(
            cache
                .depends_on(package_a, package_b)
                .expect("package not found?"),
            "{}: package '{}' should depend on '{}'",
            msg,
            &package_a.repr,
            &package_b.repr,
        ),
        DependencyDirection::Reverse => assert!(
            cache
                .depends_on(package_b, package_a)
                .expect("package not found?"),
            "{}: package '{}' should be a dependency of '{}'",
            msg,
            &package_a.repr,
            &package_b.repr,
        ),
    }
}

fn assert_not_depends_on(
    package_a: &PackageId,
    package_b: &PackageId,
    cache: &mut DependsCache,
    direction: DependencyDirection,
    msg: &str,
) {
    match direction {
        DependencyDirection::Forward => assert!(
            !cache
                .depends_on(package_a, package_b)
                .expect("package not found?"),
            "{}: package '{}' should not depend on '{}'",
            msg,
            &package_a.repr,
            &package_b.repr,
        ),
        DependencyDirection::Reverse => assert!(
            !cache
                .depends_on(package_b, package_a)
                .expect("package not found?"),
            "{}: package '{}' should not be a dependency of '{}'",
            msg,
            &package_a.repr,
            &package_b.repr,
        ),
    }
}

/// Assert that links are presented in the expected order.
///
/// For any given package not in the initial set:
/// * If direction is Forward, the package should appear in the `to` of a link at least once
///   before it appears in the `from` of a link.
/// * If direction is Reverse, the package should appear in the `from` of a link at least once
///   before it appears in the `to` of a link.
fn assert_link_order<'g>(
    links: impl IntoIterator<Item = DependencyLink<'g>>,
    initial: impl IntoIterator<Item = &'g PackageId>,
    desc: &DirectionDesc<'g>,
    msg: &str,
) {
    // for forward, 'from' is known and 'to' is variable.
    let mut variable_seen: HashSet<_> = initial.into_iter().collect();

    for link in links {
        let known_id = desc.known_metadata(&link).id();
        let variable_id = desc.variable_metadata(&link).id();

        variable_seen.insert(variable_id);
        assert!(
            variable_seen.contains(&known_id),
            "{}: for package '{}': unexpected link {} package seen before any links {} package",
            msg,
            &known_id.repr,
            desc.known_desc,
            desc.variable_desc,
        );
    }
}
