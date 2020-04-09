// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::feature::{FeatureGraph, FeatureId, FeatureMetadata};
use crate::graph::resolve_core::{ResolveCore, Topo};
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

    /// Iterates over feature IDs, in topological order in the direction specified.
    ///
    /// ## Cycles
    ///
    /// The packages within a dependency cycle will be returned in arbitrary order, but overall
    /// topological order will be maintained.
    pub fn into_ids(self, direction: DependencyDirection) -> IntoIds<'g> {
        IntoIds {
            graph: self.feature_graph,
            inner: self.core.topo(self.feature_graph.sccs(), direction),
        }
    }

    /// Iterates over feature metadatas, in topological order in the direction specified.
    ///
    /// ## Cycles
    ///
    /// The packages within a dependency cycle will be returned in arbitrary order, but overall
    /// topological order will be maintained.
    pub fn into_metadatas(self, direction: DependencyDirection) -> IntoMetadatas<'g> {
        IntoMetadatas {
            graph: self.feature_graph,
            inner: self.core.topo(self.feature_graph.sccs(), direction),
        }
    }

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

/// An iterator over feature IDs in topological order.
///
/// The items returned are of type `FeatureId<'g>`. Returned by `PackageResolveSet::into_ids`.
pub struct IntoIds<'g> {
    graph: FeatureGraph<'g>,
    inner: Topo<'g, FeatureGraph<'g>>,
}

impl<'g> IntoIds<'g> {
    /// Returns the direction the iteration is happening in.
    pub fn direction(&self) -> DependencyDirection {
        self.inner.direction()
    }
}

impl<'g> Iterator for IntoIds<'g> {
    type Item = FeatureId<'g>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|feature_ix| {
            FeatureId::from_node(
                self.graph.package_graph(),
                &self.graph.dep_graph()[feature_ix],
            )
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'g> ExactSizeIterator for IntoIds<'g> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

/// An iterator over feature metadatas in topological order.
///
/// The items returned are of type `FeatureId<'g>`. Returned by `PackageResolveSet::into_ids`.
pub struct IntoMetadatas<'g> {
    graph: FeatureGraph<'g>,
    inner: Topo<'g, FeatureGraph<'g>>,
}

impl<'g> IntoMetadatas<'g> {
    /// Returns the direction the iteration is happening in.
    pub fn direction(&self) -> DependencyDirection {
        self.inner.direction()
    }
}

impl<'g> Iterator for IntoMetadatas<'g> {
    type Item = FeatureMetadata<'g>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|feature_ix| {
            self.graph
                .metadata_for_node(&self.graph.dep_graph()[feature_ix])
                .expect("feature node should be known")
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'g> ExactSizeIterator for IntoMetadatas<'g> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}
