// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::feature::{FeatureGraph, FeatureId};
use crate::graph::{compute_roots, DependencyDirection, PackageSelect, SelectParams};
use crate::Error;
use std::collections::HashSet;

/// Trait representing whether a feature within a package should be selected.
///
/// This is conceptually similar to passing `--features` or other similar command-line options to
/// Cargo.
///
/// Most uses will involve using one of the predefined filters: `all_filter`, `default_filter`, or
/// `none_filter`. For advanced uses, the trait is implemented for all functions that match
/// `for<'g> FnMut(&FeatureGraph<'g>, FeatureId<'g>) -> bool`. The `filter_fn` helper is provided to
/// assist with type inference.
pub trait FeatureFilter {
    /// Returns true if this feature ID should be selected in the graph.
    ///
    /// Returning false does not prevent this feature ID from being included if it's reachable
    /// through other means.
    ///
    /// In general, `accept` should return true if `feature_id.is_base()` is true.
    ///
    /// The feature ID is guaranteed to be in this graph, so it is OK to panic if it isn't found.
    fn accept(&mut self, graph: &FeatureGraph<'_>, feature_id: FeatureId<'_>) -> bool;
}

impl<F> FeatureFilter for F
where
    F: for<'g> FnMut(&FeatureGraph<'g>, FeatureId<'g>) -> bool,
{
    fn accept(&mut self, graph: &FeatureGraph<'_>, feature_id: FeatureId<'_>) -> bool {
        self(graph, feature_id)
    }
}

impl<'a> FeatureFilter for Box<dyn FeatureFilter + 'a> {
    fn accept(&mut self, graph: &FeatureGraph<'_>, feature_id: FeatureId<'_>) -> bool {
        (**self).accept(graph, feature_id)
    }
}

impl<'a> FeatureFilter for &'a mut dyn FeatureFilter {
    fn accept(&mut self, graph: &FeatureGraph<'_>, feature_id: FeatureId<'_>) -> bool {
        (**self).accept(graph, feature_id)
    }
}

/// Returns a `FeatureFilter` which simply calls the function that's passed in.
///
/// This is a no-op and is not strictly necessary, but can assist with type inference.
pub fn filter_fn<F>(filter_fn: F) -> impl FeatureFilter
where
    F: for<'g> FnMut(&FeatureGraph<'_>, FeatureId<'g>) -> bool,
{
    filter_fn
}

/// Returns a `FeatureFilter` that selects all features from the given packages.
///
/// This is equivalent to a build with `--all-features`.
pub fn all_filter() -> impl FeatureFilter {
    filter_fn(|_, _| true)
}

/// Returns a `FeatureFilter` that selects no features from the given packages.
///
/// This is equivalent to a build with `--no-default-features`.
pub fn none_filter() -> impl FeatureFilter {
    filter_fn(|_, feature_id| {
        // The only feature ID that should be accepted is the base one.
        feature_id.is_base()
    })
}

/// Returns a `FeatureFilter` that selects default features from the given packages.
///
/// This is equivalent to a standard `cargo build`.
pub fn default_filter() -> impl FeatureFilter {
    filter_fn(|feature_graph, feature_id| {
        // XXX it kinda sucks that we already know about the exact feature ixs but need to go
        // through the feature ID over here. Might be worth reorganizing the code to not do that.
        feature_graph
            .is_default_feature(feature_id)
            .expect("feature IDs should be valid")
    })
}

/// Returns a `FeatureFilter` that selects everything from the base filter, plus these additional
/// feature names -- regardless of what package they are in.
///
/// This is equivalent to a build with `--features`, and is typically meant to be used with one
/// package.
///
/// For filtering by feature IDs, use `feature_id_filter`.
pub fn feature_filter<'a>(
    base: impl FeatureFilter + 'a,
    features: impl IntoIterator<Item = &'a str>,
) -> impl FeatureFilter + 'a {
    let mut base = base;
    let features: HashSet<_> = features.into_iter().collect();
    filter_fn(move |feature_graph, feature_id| {
        if base.accept(feature_graph, feature_id) {
            return true;
        }
        match feature_id.feature() {
            Some(feature) => features.contains(feature),
            None => {
                // This is the base feature. Assume that it has already been selected by the base
                // filter.
                false
            }
        }
    })
}

/// Returns a `FeatureFilter` that selects everything from the base filter, plus some additional
/// feature IDs.
///
/// This is a more advanced version of `feature_filter`.
pub fn feature_id_filter<'a>(
    base: impl FeatureFilter + 'a,
    feature_ids: impl IntoIterator<Item = impl Into<FeatureId<'a>>>,
) -> impl FeatureFilter + 'a {
    let mut base = base;
    let feature_ids: HashSet<_> = feature_ids
        .into_iter()
        .map(|feature_id| feature_id.into())
        .collect();
    filter_fn(move |feature_graph, feature_id| {
        base.accept(feature_graph, feature_id) || feature_ids.contains(&feature_id)
    })
}

/// A selector over a feature graph.
///
/// This is the entry point for iterators overs IDs and dependency links, and dot graph presentation.
/// A `FeatureSelect` is constructed through the `select_` methods on `FeatureGraph`.
#[derive(Clone, Debug)]
pub struct FeatureSelect<'g> {
    graph: FeatureGraph<'g>,
    pub(super) params: SelectParams<FeatureGraph<'g>>,
}

/// ## Selectors
///
/// The methods in this section create *feature selectors*, which are queries over subsets of this
/// feature graph. Use the methods here for queries based on transitive dependencies.
impl<'g> FeatureGraph<'g> {
    /// Creates a new selector over the entire workspace.
    ///
    /// `select_workspace` will select all workspace packages (subject to the provided filter) and
    /// their transitive dependencies.
    pub fn select_workspace(&self, filter: impl FeatureFilter) -> FeatureSelect<'g> {
        self.select_packages(&self.package_graph.select_workspace(), filter)
    }

    /// Creates a new selector that returns all members of this feature graph.
    ///
    /// This will include features that aren't depended on by any workspace packages.
    ///
    /// In most situations, `select_workspace` is preferred. Use `select_all` if you know you need
    /// parts of the graph that aren't accessible from the workspace.
    pub fn select_all(&self) -> FeatureSelect<'g> {
        self.select_packages(&self.package_graph.select_all(), all_filter())
    }

    /// Creates a new selector for all packages selected through this `PackageSelect` instance.
    ///
    /// If `select_all` is passed in, the filter is ignored.
    pub fn select_packages(
        &self,
        packages: &PackageSelect<'g>,
        filter: impl FeatureFilter,
    ) -> FeatureSelect<'g> {
        let params = match &packages.params {
            SelectParams::All => {
                // The filter is ignored -- there's no real sensible way to apply it.
                SelectParams::All
            }
            SelectParams::SelectForward(package_ixs) => SelectParams::SelectForward(
                self.feature_ixs_for_packages(package_ixs.iter().copied(), filter)
                    .collect(),
            ),
            SelectParams::SelectReverse(package_ixs) => SelectParams::SelectReverse(
                self.feature_ixs_for_packages(package_ixs.iter().copied(), filter)
                    .collect(),
            ),
        };

        FeatureSelect {
            graph: *self,
            params,
        }
    }

    /// Creates a new selector that returns transitive dependencies of the given feature IDs in the
    /// specified direction.
    ///
    /// Returns an error if any feature IDs are unknown.
    pub fn select_directed<'a>(
        &self,
        feature_ids: impl IntoIterator<Item = impl Into<FeatureId<'a>>>,
        dep_direction: DependencyDirection,
    ) -> Result<FeatureSelect<'g>, Error> {
        match dep_direction {
            DependencyDirection::Forward => self.select_forward(feature_ids),
            DependencyDirection::Reverse => self.select_reverse(feature_ids),
        }
    }

    /// Creates a new selector that returns transitive dependencies of the given feature IDs.
    ///
    /// Returns an error if any feature IDs are unknown.
    pub fn select_forward<'a>(
        &self,
        feature_ids: impl IntoIterator<Item = impl Into<FeatureId<'a>>>,
    ) -> Result<FeatureSelect<'g>, Error> {
        let feature_ids = feature_ids.into_iter().map(|feature_id| feature_id.into());
        Ok(FeatureSelect {
            graph: *self,
            params: SelectParams::SelectForward(self.feature_ixs(feature_ids)?),
        })
    }

    /// Creates a new selector that returns transitive reverse dependencies of the given feature IDs.
    ///
    /// Returns an error if any feature IDs are unknown.
    pub fn select_reverse<'a>(
        &self,
        feature_ids: impl IntoIterator<Item = impl Into<FeatureId<'a>>>,
    ) -> Result<FeatureSelect<'g>, Error> {
        let feature_ids = feature_ids.into_iter().map(|feature_id| feature_id.into());
        Ok(FeatureSelect {
            graph: *self,
            params: SelectParams::SelectReverse(self.feature_ixs(feature_ids)?),
        })
    }
}

impl<'g> FeatureSelect<'g> {
    /// Returns the set of "root feature IDs" in the specified direction.
    ///
    /// * If direction is Forward, return the set of feature IDs that do not have any dependencies
    ///   within the selected graph.
    /// * If direction is Reverse, return the set of feature IDs that do not have any dependents
    ///   within the selected graph.
    pub fn into_root_ids(
        self,
        direction: DependencyDirection,
    ) -> impl Iterator<Item = FeatureId<'g>> + 'g {
        let dep_graph = self.graph.dep_graph();
        let package_graph = self.graph.package_graph;
        compute_roots(dep_graph, self.params, direction)
            .into_iter()
            .map(move |feature_ix| FeatureId::from_node(package_graph, &dep_graph[feature_ix]))
    }
}
