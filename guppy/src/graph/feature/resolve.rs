// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::feature::{FeatureGraph, FeatureId, FeatureMetadata};
use crate::graph::resolve_core::ResolveCore;
use crate::graph::select_core::SelectParams;
use crate::graph::DependencyDirection;

/// A set of resolved feature IDs in a feature graph.
///
/// Created by `FeatureSelect::resolve`.
#[derive(Clone, Debug)]
pub struct FeatureSet<'g> {
    feature_graph: FeatureGraph<'g>,
    core: ResolveCore<FeatureGraph<'g>>,
}

impl<'g> FeatureSet<'g> {
    pub(super) fn new(
        feature_graph: FeatureGraph<'g>,
        params: SelectParams<FeatureGraph<'g>>,
    ) -> Self {
        Self {
            feature_graph,
            core: ResolveCore::new(feature_graph.dep_graph(), params),
        }
    }

    /// Returns the number of feature IDs in this set.
    pub fn len(&self) -> usize {
        self.core.len()
    }

    /// Returns true if no feature IDs were resolved in this set.
    pub fn is_empty(&self) -> bool {
        self.core.is_empty()
    }

    /// Returns true if this set contains the given feature ID, false if it doesn't, or None if it
    /// wasn't found.
    pub fn contains<'a>(&self, feature_id: impl Into<FeatureId<'a>>) -> Option<bool> {
        Some(
            self.core
                .contains(self.feature_graph.feature_ix(feature_id.into())?),
        )
    }

    // ---
    // Set operations
    // ---

    /// Returns a `PackageSet` that contains all packages present in at least one of `self`
    /// and `other`.
    ///
    /// ## Panics
    ///
    /// Panics if the package graphs associated with `self` and `other` don't match.
    pub fn union(&self, other: &Self) -> Self {
        assert!(
            ::std::ptr::eq(
                self.feature_graph.package_graph,
                self.feature_graph.package_graph
            ),
            "package graphs passed into union() match"
        );
        let mut res = self.clone();
        res.core.union_with(&other.core);
        res
    }

    /// Returns a `PackageSet` that contains all packages present in both `self` and `other`.
    ///
    /// ## Panics
    ///
    /// Panics if the package graphs associated with `self` and `other` don't match.
    pub fn intersection(&self, other: &Self) -> Self {
        assert!(
            ::std::ptr::eq(
                self.feature_graph.package_graph,
                self.feature_graph.package_graph
            ),
            "package graphs passed into intersection() match"
        );
        let mut res = self.clone();
        res.core.intersect_with(&other.core);
        res
    }

    /// Returns a `PackageSet` that contains all packages present in `self` but not `other`.
    ///
    /// ## Panics
    ///
    /// Panics if the package graphs associated with `self` and `other` don't match.
    pub fn difference(&self, other: &Self) -> Self {
        assert!(
            ::std::ptr::eq(
                self.feature_graph.package_graph,
                self.feature_graph.package_graph
            ),
            "package graphs passed into difference() match"
        );
        Self {
            feature_graph: self.feature_graph,
            core: self.core.difference(&other.core),
        }
    }

    /// Returns a `PackageSet` that contains all packages present in exactly one of `self` and
    /// `other`.
    ///
    /// ## Panics
    ///
    /// Panics if the package graphs associated with `self` and `other` don't match.
    pub fn symmetric_difference(&self, other: &Self) -> Self {
        assert!(
            ::std::ptr::eq(
                self.feature_graph.package_graph,
                self.feature_graph.package_graph
            ),
            "package graphs passed into symmetric_difference() match"
        );
        let mut res = self.clone();
        res.core.symmetric_difference_with(&other.core);
        res
    }

    // ---
    // Iterators
    // ---

    /// Returns the set of "root feature" IDs in the specified direction.
    ///
    /// * If direction is Forward, return the set of feature IDs that do not have any dependencies
    ///   within the selected graph.
    /// * If direction is Reverse, return the set of feature IDs that do not have any dependents
    ///   within the selected graph.
    ///
    /// ## Cycles
    ///
    /// If a root consists of a dependency cycle, all the packages in it will be returned in
    /// arbitrary order.
    pub fn into_root_ids(
        self,
        direction: DependencyDirection,
    ) -> impl Iterator<Item = FeatureId<'g>> + 'g {
        let dep_graph = self.feature_graph.dep_graph();
        let package_graph = self.feature_graph.package_graph;
        self.core
            .roots(dep_graph, self.feature_graph.sccs(), direction)
            .into_iter()
            .map(move |feature_ix| FeatureId::from_node(package_graph, &dep_graph[feature_ix]))
    }

    /// Returns the set of "root feature" metadatas in the specified direction.
    ///
    /// * If direction is Forward, return the set of metadatas that do not have any dependencies
    ///   within the selected graph.
    /// * If direction is Reverse, return the set of metadatas that do not have any dependents
    ///   within the selected graph.
    ///
    /// ## Cycles
    ///
    /// If a root consists of a dependency cycle, all the packages in it will be returned in
    /// arbitrary order.}
    pub fn into_root_metadatas(
        self,
        direction: DependencyDirection,
    ) -> impl Iterator<Item = FeatureMetadata<'g>> + 'g {
        let feature_graph = self.feature_graph;
        self.core
            .roots(feature_graph.dep_graph(), feature_graph.sccs(), direction)
            .into_iter()
            .map(move |feature_ix| {
                let feature_node = &feature_graph.dep_graph()[feature_ix];
                feature_graph
                    .metadata_for_node(feature_node)
                    .expect("feature node should be known")
            })
    }
}
