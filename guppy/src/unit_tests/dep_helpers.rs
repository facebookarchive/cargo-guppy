// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::feature::{FeatureGraph, FeatureId, FeatureMetadata, FeatureSet};
use crate::graph::{
    kind_str, DependencyDirection, DependencyEdge, DependencyLink, DependencyMetadata,
    PackageGraph, PackageMetadata, PackageSet,
};
use crate::unit_tests::fixtures::PackageDetails;
use crate::{DependencyKind, Error, PackageId};
use pretty_assertions::assert_eq;
use std::collections::{BTreeSet, HashSet};
use std::fmt;
use std::hash::Hash;
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
#[derive(Clone, Copy)]
pub(crate) struct DirectionDesc<'a> {
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

impl<'a> From<DependencyDirection> for DirectionDesc<'a> {
    fn from(direction: DependencyDirection) -> Self {
        Self::new(direction)
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

    for (_, _, dep_id) in &actual_dep_ids {
        // depends_on should agree with the dependencies returned.
        graph.assert_depends_on(known_details.id(), dep_id, direction, msg);
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

    let query = graph
        .query_directed(iter::once(known_details.id()), direction)
        .unwrap_or_else(|err| {
            panic!(
                "{}: {} transitive dep query failed: {}",
                msg, desc.direction_desc, err
            )
        });
    let package_ids = query.clone().resolve().into_ids(direction);
    let mut actual_dep_ids: Vec<_> = package_ids.collect();
    actual_dep_ids.sort();

    let actual_deps: Vec<_> = query.clone().resolve().into_links(direction).collect();
    let actual_ptrs = dep_link_ptrs(actual_deps.iter().copied());

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
    assert_link_order(
        actual_deps,
        query.clone().resolve().into_root_ids(direction),
        desc,
        &format!("{}: actual link order", msg),
    );

    // Do a query in the opposite direction as well to test link order.
    let opposite = direction.opposite();
    let opposite_desc = DirectionDesc::new(opposite);
    let opposite_deps: Vec<_> = query.clone().resolve().into_links(opposite).collect();
    let opposite_ptrs = dep_link_ptrs(opposite_deps.iter().copied());

    // Checking for pointer equivalence is enough since they both use the same graph as a base.
    assert_eq!(
        actual_ptrs, opposite_ptrs,
        "{}: actual and opposite links should return the same pointer triples",
        msg,
    );

    assert_link_order(
        opposite_deps,
        query.resolve().into_root_ids(opposite),
        opposite_desc,
        &format!("{}: opposite link order", msg),
    );

    for dep_id in expected_dep_id_refs {
        // depends_on should agree with this.
        graph.assert_depends_on(known_details.id(), dep_id, direction, msg);

        // Transitive deps should be transitively closed.
        let dep_actual_dep_ids: BTreeSet<_> = graph
            .query_directed(iter::once(dep_id), direction)
            .unwrap_or_else(|err| {
                panic!(
                    "{}: {} transitive dep id query failed for dependency '{}': {}",
                    msg, desc.direction_desc, dep_id.repr, err
                )
            })
            .resolve()
            .into_ids(direction)
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
            .query_directed(iter::once(dep_id), direction)
            .unwrap_or_else(|err| {
                panic!(
                    "{}: {} transitive dep query failed for dependency '{}': {}",
                    msg, desc.direction_desc, dep_id.repr, err
                )
            })
            .resolve()
            .into_links(direction)
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
    let topo_ids = graph.resolve_all().into_ids(direction);
    assert_eq!(
        topo_ids.len(),
        graph.package_count(),
        "{}: topo sort returns all packages",
        msg
    );

    // A package that comes later cannot depend on one that comes earlier.
    graph.assert_topo_order(topo_ids, direction, msg);
}

pub(crate) fn assert_topo_metadatas(
    graph: &PackageGraph,
    direction: DependencyDirection,
    msg: &str,
) {
    let topo_metadatas = graph.resolve_all().into_metadatas(direction);
    assert_eq!(
        topo_metadatas.len(),
        graph.package_count(),
        "{}: topo sort returns all packages",
        msg
    );
    let topo_ids = topo_metadatas.map(|metadata| metadata.id());

    // A package that comes later cannot depend on one that comes earlier.
    graph.assert_topo_order(topo_ids, direction, msg);
}

pub(crate) fn assert_all_links(graph: &PackageGraph, direction: DependencyDirection, msg: &str) {
    let desc = DirectionDesc::new(direction);
    let all_links: Vec<_> = graph.resolve_all().into_links(direction).collect();
    assert_eq!(
        all_links.len(),
        graph.link_count(),
        "{}: all links should be returned",
        msg
    );

    // The enabled status can't be unknown on the current platform.
    for DependencyLink { from, to, edge } in &all_links {
        for dep_kind in &[
            DependencyKind::Normal,
            DependencyKind::Build,
            DependencyKind::Development,
        ] {
            assert_enabled_status_is_known(
                edge.metadata_for_kind(*dep_kind),
                &format!(
                    "{}: {} -> {} ({})",
                    msg,
                    from.id(),
                    to.id(),
                    kind_str(*dep_kind)
                ),
            );
        }
    }

    // all_links should be in the correct order.
    assert_link_order(
        all_links,
        graph.resolve_all().into_root_ids(direction),
        desc,
        msg,
    );
}

fn assert_enabled_status_is_known(metadata: Option<&DependencyMetadata>, msg: &str) {
    let metadata = match metadata {
        Some(metadata) => metadata,
        None => return,
    };

    assert!(
        metadata.enabled().is_known(),
        "{}: enabled status known for current platform",
        msg
    );
    assert!(
        metadata.default_features().is_known(),
        "{}: default feature status known for current platform",
        msg
    );
    for feature in metadata.features() {
        assert!(
            metadata.feature_enabled(feature).is_known(),
            "{}: for feature '{}', status known for current platform",
            msg,
            feature
        );
    }
}

pub(super) trait GraphAssert<'g>: Copy + fmt::Debug {
    type Id: Copy + Eq + Hash + fmt::Debug;
    type Metadata: GraphMetadata<'g, Id = Self::Id>;
    type Set: GraphSet<'g, Id = Self::Id, Metadata = Self::Metadata>;
    const NAME: &'static str;

    // TODO: Add support for checks around links once they're defined for feature graphs.

    fn depends_on(&self, a_id: Self::Id, b_id: Self::Id) -> Result<bool, Error>;

    fn is_cyclic(&self, a_id: Self::Id, b_id: Self::Id) -> Result<bool, Error>;

    fn resolve(&self, initials: &[Self::Id], direction: DependencyDirection) -> Self::Set;

    fn ids(
        &self,
        initials: &[Self::Id],
        query_direction: DependencyDirection,
        iter_direction: DependencyDirection,
    ) -> Vec<Self::Id> {
        let package_set = self.resolve(initials, query_direction);
        let resolve_len = package_set.len();
        let ids = package_set.ids(iter_direction);
        assert_eq!(resolve_len, ids.len(), "resolve.len() is correct");
        ids
    }

    fn root_ids(
        &self,
        initials: &[Self::Id],
        query_direction: DependencyDirection,
        iter_direction: DependencyDirection,
    ) -> Vec<Self::Id> {
        self.resolve(initials, query_direction)
            .root_ids(iter_direction)
    }

    fn root_metadatas(
        &self,
        initials: &[Self::Id],
        query_direction: DependencyDirection,
        iter_direction: DependencyDirection,
    ) -> Vec<Self::Metadata> {
        self.resolve(initials, query_direction)
            .root_metadatas(iter_direction)
    }

    fn assert_topo_order<'a>(
        &self,
        topo_ids: impl IntoIterator<Item = Self::Id>,
        direction: DependencyDirection,
        msg: &str,
    ) {
        let topo_ids: Vec<_> = topo_ids.into_iter().collect();
        for (idx, earlier_package) in topo_ids.iter().enumerate() {
            // Note that this skips over idx + 1 entries to avoid earlier_package == later_package.
            // Doing an exhaustive search would be O(n**2) in the number of packages, so just do a
            // maximum of 20.
            // TODO: use proptest to generate random queries on the corpus.
            for later_package in topo_ids.iter().skip(idx + 1).take(20) {
                self.assert_not_depends_on(*later_package, *earlier_package, direction, msg);
            }
        }
    }

    fn assert_depends_on_any(
        &self,
        source_ids: &[Self::Id],
        query_id: Self::Id,
        direction: DependencyDirection,
        msg: &str,
    ) {
        let any_depends_on = source_ids.iter().any(|source_id| match direction {
            DependencyDirection::Forward => self.depends_on(*source_id, query_id).unwrap(),
            DependencyDirection::Reverse => self.depends_on(query_id, *source_id).unwrap(),
        });
        match direction {
            DependencyDirection::Forward => {
                assert!(
                    any_depends_on,
                    "{}: {} '{:?}' should be a dependency of any of '{:?}'",
                    msg,
                    Self::NAME,
                    query_id,
                    source_ids
                );
            }
            DependencyDirection::Reverse => {
                assert!(
                    any_depends_on,
                    "{}: {} '{:?}' should depend on any of '{:?}'",
                    msg,
                    Self::NAME,
                    query_id,
                    source_ids
                );
            }
        }
    }

    fn assert_depends_on(
        &self,
        a_id: Self::Id,
        b_id: Self::Id,
        direction: DependencyDirection,
        msg: &str,
    ) {
        match direction {
            DependencyDirection::Forward => assert!(
                self.depends_on(a_id, b_id).unwrap(),
                "{}: {} '{:?}' should depend on '{:?}'",
                msg,
                Self::NAME,
                a_id,
                b_id,
            ),
            DependencyDirection::Reverse => assert!(
                self.depends_on(b_id, a_id).unwrap(),
                "{}: {} '{:?}' should be a dependency of '{:?}'",
                msg,
                Self::NAME,
                a_id,
                b_id,
            ),
        }
    }

    fn assert_not_depends_on(
        &self,
        a_id: Self::Id,
        b_id: Self::Id,
        direction: DependencyDirection,
        msg: &str,
    ) {
        if self.is_cyclic(a_id, b_id).unwrap() {
            // This is a dependency cycle -- ignore it in not-depends-on checks.
            return;
        }

        match direction {
            DependencyDirection::Forward => assert!(
                !self.depends_on(a_id, b_id).unwrap(),
                "{}: {} '{:?}' should not depend on '{:?}'",
                msg,
                Self::NAME,
                a_id,
                b_id,
            ),
            DependencyDirection::Reverse => assert!(
                !self.depends_on(b_id, a_id).unwrap(),
                "{}: {} '{:?}' should not be a dependency of '{:?}'",
                msg,
                Self::NAME,
                a_id,
                b_id,
            ),
        }
    }
}

pub(super) trait GraphMetadata<'g> {
    type Id: Copy + Eq + Hash + fmt::Debug;
    fn id(&self) -> Self::Id;
}

pub(super) trait GraphSet<'g>: Clone + fmt::Debug {
    type Id: Copy + Eq + Hash + fmt::Debug;
    type Metadata: GraphMetadata<'g, Id = Self::Id>;
    fn len(&self) -> usize;
    fn contains(&self, id: Self::Id) -> bool;

    fn union(&self, other: &Self) -> Self;
    fn intersection(&self, other: &Self) -> Self;
    fn difference(&self, other: &Self) -> Self;
    fn symmetric_difference(&self, other: &Self) -> Self;

    fn ids(self, direction: DependencyDirection) -> Vec<Self::Id>;
    fn metadatas(self, direction: DependencyDirection) -> Vec<Self::Metadata>;
    fn root_ids(self, direction: DependencyDirection) -> Vec<Self::Id>;
    fn root_metadatas(self, direction: DependencyDirection) -> Vec<Self::Metadata>;
}

impl<'g> GraphAssert<'g> for &'g PackageGraph {
    type Id = &'g PackageId;
    type Metadata = &'g PackageMetadata;
    type Set = PackageSet<'g>;
    const NAME: &'static str = "package";

    fn depends_on(&self, a_id: Self::Id, b_id: Self::Id) -> Result<bool, Error> {
        PackageGraph::depends_on(self, a_id, b_id)
    }

    fn is_cyclic(&self, a_id: Self::Id, b_id: Self::Id) -> Result<bool, Error> {
        let cycles = self.cycles();
        cycles.is_cyclic(a_id, b_id)
    }

    fn resolve(&self, initials: &[Self::Id], direction: DependencyDirection) -> Self::Set {
        self.query_directed(initials.iter().copied(), direction)
            .unwrap()
            .resolve()
    }
}

impl<'g> GraphMetadata<'g> for &'g PackageMetadata {
    type Id = &'g PackageId;
    fn id(&self) -> Self::Id {
        PackageMetadata::id(self)
    }
}

impl<'g> GraphSet<'g> for PackageSet<'g> {
    type Id = &'g PackageId;
    type Metadata = &'g PackageMetadata;

    fn len(&self) -> usize {
        self.len()
    }

    fn contains(&self, id: Self::Id) -> bool {
        self.contains(id).unwrap()
    }

    fn union(&self, other: &Self) -> Self {
        self.union(other)
    }

    fn intersection(&self, other: &Self) -> Self {
        self.intersection(other)
    }

    fn difference(&self, other: &Self) -> Self {
        self.difference(other)
    }

    fn symmetric_difference(&self, other: &Self) -> Self {
        self.symmetric_difference(other)
    }

    fn ids(self, direction: DependencyDirection) -> Vec<Self::Id> {
        self.into_ids(direction).collect()
    }

    fn metadatas(self, direction: DependencyDirection) -> Vec<Self::Metadata> {
        self.into_metadatas(direction).collect()
    }

    fn root_ids(self, direction: DependencyDirection) -> Vec<Self::Id> {
        self.into_root_ids(direction).collect()
    }

    fn root_metadatas(self, direction: DependencyDirection) -> Vec<Self::Metadata> {
        self.into_root_metadatas(direction).collect()
    }
}

impl<'g> GraphAssert<'g> for FeatureGraph<'g> {
    type Id = FeatureId<'g>;
    type Metadata = FeatureMetadata<'g>;
    type Set = FeatureSet<'g>;
    const NAME: &'static str = "feature";

    fn depends_on(&self, a_id: Self::Id, b_id: Self::Id) -> Result<bool, Error> {
        FeatureGraph::depends_on(self, a_id, b_id)
    }

    fn is_cyclic(&self, a_id: Self::Id, b_id: Self::Id) -> Result<bool, Error> {
        let cycles = self.cycles();
        cycles.is_cyclic(a_id, b_id)
    }

    fn resolve(&self, initials: &[Self::Id], direction: DependencyDirection) -> Self::Set {
        self.query_directed(initials.iter().copied(), direction)
            .unwrap()
            .resolve()
    }
}

impl<'g> GraphMetadata<'g> for FeatureMetadata<'g> {
    type Id = FeatureId<'g>;
    fn id(&self) -> Self::Id {
        self.feature_id()
    }
}

impl<'g> GraphSet<'g> for FeatureSet<'g> {
    type Id = FeatureId<'g>;
    type Metadata = FeatureMetadata<'g>;

    fn len(&self) -> usize {
        self.len()
    }

    fn contains(&self, id: Self::Id) -> bool {
        self.contains(id).unwrap()
    }

    fn union(&self, other: &Self) -> Self {
        self.union(other)
    }

    fn intersection(&self, other: &Self) -> Self {
        self.intersection(other)
    }

    fn difference(&self, other: &Self) -> Self {
        self.difference(other)
    }

    fn symmetric_difference(&self, other: &Self) -> Self {
        self.symmetric_difference(other)
    }

    fn ids(self, direction: DependencyDirection) -> Vec<Self::Id> {
        self.into_ids(direction).collect()
    }

    fn metadatas(self, direction: DependencyDirection) -> Vec<Self::Metadata> {
        self.into_metadatas(direction).collect()
    }

    fn root_ids(self, direction: DependencyDirection) -> Vec<Self::Id> {
        self.into_root_ids(direction).collect()
    }

    fn root_metadatas(self, direction: DependencyDirection) -> Vec<Self::Metadata> {
        self.into_root_metadatas(direction).collect()
    }
}

/// Assert that links are presented in the expected order.
///
/// For any given package not in the initial set:
/// * If direction is Forward, the package should appear in the `to` of a link at least once
///   before it appears in the `from` of a link.
/// * If direction is Reverse, the package should appear in the `from` of a link at least once
///   before it appears in the `to` of a link.
pub(crate) fn assert_link_order<'g>(
    links: impl IntoIterator<Item = DependencyLink<'g>>,
    initial: impl IntoIterator<Item = &'g PackageId>,
    desc: impl Into<DirectionDesc<'g>>,
    msg: &str,
) {
    let desc = desc.into();

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

fn dep_link_ptrs<'g>(
    dep_links: impl IntoIterator<Item = DependencyLink<'g>>,
) -> Vec<(
    *const PackageMetadata,
    *const PackageMetadata,
    *const DependencyEdge,
)> {
    let mut triples: Vec<_> = dep_links
        .into_iter()
        .map(|link| {
            (
                link.from as *const _,
                link.to as *const _,
                link.edge as *const _,
            )
        })
        .collect();
    triples.sort();
    triples
}
