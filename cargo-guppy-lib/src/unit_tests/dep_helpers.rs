// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::{DependencyDirection, DependencyLink, PackageGraph, PackageMetadata};
use crate::unit_tests::fixtures::PackageDetails;
use cargo_metadata::PackageId;
use std::collections::{BTreeSet, HashMap};
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
#[allow(dead_code)]
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
        .map(|(dep_name, id)| (*dep_name, dep_name.replace("-", "_"), *id))
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
                desc.variable_metadata(&dep).id().repr.as_str(),
            )
        })
        .collect();
    actual_dep_ids.sort();
    assert_eq!(
        expected_dep_ids, actual_dep_ids,
        "{}: expected {} dependencies",
        msg, desc.direction_desc,
    );

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

    // Transitive deps should be in topological order.
    assert_topo_order(actual_deps.iter().copied(), direction, msg);

    // Transitive deps should be transitively closed.
    for dep_id in expected_dep_id_refs {
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

/// Assert that the links provided are in topological order.
///
/// For any given package:
/// * If direction is Forward, the package should never appear in the `to` of a link after it
///   appears in the `from` of a link.
/// * If direction is Reverse, the package should never appear in the `from` of a link after it
///   appears in the `to` of a link.
pub(crate) fn assert_topo_order<'g: 'a, 'a>(
    links: impl IntoIterator<Item = DependencyLink<'g>>,
    direction: DependencyDirection,
    msg: &str,
) {
    // The package should never appear in known_metadata after it appears in variable_metadata.
    let mut check_states = HashMap::new();
    let desc = DirectionDesc::new(direction);
    for link in links {
        let known_id = desc.known_metadata(&link).id();
        let variable_id = desc.variable_metadata(&link).id();
        println!(
            "link from: {} to: {}: known: {}, variable: {}",
            link.from.id().repr,
            link.to.id().repr,
            known_id.repr,
            variable_id.repr
        );
        check_states
            .entry(known_id)
            .or_insert_with(|| TopoCheckState::new(known_id))
            .record_phase2(variable_id);
        check_states
            .entry(variable_id)
            .or_insert_with(|| TopoCheckState::new(variable_id))
            .record_phase1(known_id, &desc, msg);
    }
}

/// This struct has two states: "phase 1", in which `package_id` has been seen on the
/// variable end of links, and "phase 2", in which `package_id` has been seen on the known
/// end of at least one link. Packages can move from phase 1 to phase 2 but not back.
#[derive(Debug)]
struct TopoCheckState<'a> {
    package_id: &'a PackageId,
    phase1_seen: Vec<&'a PackageId>,
    phase2_seen: Vec<&'a PackageId>,
}

impl<'a> TopoCheckState<'a> {
    fn new(package_id: &'a PackageId) -> Self {
        Self {
            package_id,
            phase1_seen: vec![],
            phase2_seen: vec![],
        }
    }

    fn record_phase1(&mut self, known_id: &'a PackageId, desc: &DirectionDesc, msg: &str) {
        match self.phase2_seen.is_empty() {
            true => self.phase1_seen.push(known_id),
            false => panic!(
                "{}: for package P = '{}', unexpected link {known_desc} '{}' {variable_desc} P \
                 after links {known_desc} P (previously seen: \
                 links {variable_desc} P: {:?}, links {known_desc} P: {:?}",
                msg,
                self.package_id.repr,
                known_id.repr,
                self.phase1_seen,
                self.phase2_seen,
                known_desc = desc.known_desc,
                variable_desc = desc.variable_desc,
            ),
        }
    }

    fn record_phase2(&mut self, variable_id: &'a PackageId) {
        // phase2_seen not being empty indicates that this package has moved to phase2.
        self.phase2_seen.push(variable_id);
    }
}
