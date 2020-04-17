// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::graph::feature::{FeatureGraph, FeatureId, FeatureSet};
use crate::graph::query_core::QueryParams;
use crate::graph::{DependencyDirection, PackageQuery};
use crate::Error;
use std::collections::HashSet;

/// Trait representing whether a feature within a package should be selected.
///
/// This is conceptually similar to passing `--features` or other similar command-line options to
/// Cargo.
///
/// Most uses will involve using one of the predefined filters: `all_filter`, `default_filter`, or
/// `none_filter`. A customized filter can be provided either through `filter_fn` or by implementing
/// this trait.
pub trait FeatureFilter<'g> {
    /// Returns true if this feature ID should be selected in the graph.
    ///
    /// Returning false does not prevent this feature ID from being included if it's reachable
    /// through other means.
    ///
    /// In general, `accept` should return true if `feature_id.is_base()` is true.
    ///
    /// The feature ID is guaranteed to be in this graph, so it is OK to panic if it isn't found.
    fn accept(&mut self, graph: &FeatureGraph<'g>, feature_id: FeatureId<'g>) -> bool;
}

impl<'g, 'a, T> FeatureFilter<'g> for &'a mut T
where
    T: FeatureFilter<'g>,
{
    fn accept(&mut self, graph: &FeatureGraph<'g>, feature_id: FeatureId<'g>) -> bool {
        (**self).accept(graph, feature_id)
    }
}

impl<'g, 'a> FeatureFilter<'g> for Box<dyn FeatureFilter<'g> + 'a> {
    fn accept(&mut self, graph: &FeatureGraph<'g>, feature_id: FeatureId<'g>) -> bool {
        (**self).accept(graph, feature_id)
    }
}

impl<'g, 'a> FeatureFilter<'g> for &'a mut dyn FeatureFilter<'g> {
    fn accept(&mut self, graph: &FeatureGraph<'g>, feature_id: FeatureId<'g>) -> bool {
        (**self).accept(graph, feature_id)
    }
}

/// A `FeatureFilter` which calls the function that's passed in.
#[derive(Clone, Debug)]
pub struct FeatureFilterFn<F>(F);

impl<'g, F> FeatureFilterFn<F>
where
    F: FnMut(&FeatureGraph<'g>, FeatureId<'g>) -> bool,
{
    /// Returns a new instance of this wrapper.
    pub fn new(f: F) -> Self {
        FeatureFilterFn(f)
    }
}

impl<'g, F> FeatureFilter<'g> for FeatureFilterFn<F>
where
    F: FnMut(&FeatureGraph<'g>, FeatureId<'g>) -> bool,
{
    fn accept(&mut self, graph: &FeatureGraph<'g>, feature_id: FeatureId<'g>) -> bool {
        (self.0)(graph, feature_id)
    }
}

/// Returns a `FeatureFilter` that selects all features from the given packages.
///
/// This is equivalent to a build with `--all-features`.
pub fn all_filter<'g>() -> impl FeatureFilter<'g> {
    FeatureFilterFn::new(|_, _| true)
}

/// Returns a `FeatureFilter` that selects no features from the given packages.
///
/// This is equivalent to a build with `--no-default-features`.
pub fn none_filter<'g>() -> impl FeatureFilter<'g> {
    FeatureFilterFn::new(|_, feature_id| {
        // The only feature ID that should be accepted is the base one.
        feature_id.is_base()
    })
}

/// Returns a `FeatureFilter` that selects default features from the given packages.
///
/// This is equivalent to a standard `cargo build`.
pub fn default_filter<'g>() -> impl FeatureFilter<'g> {
    FeatureFilterFn::new(|feature_graph, feature_id| {
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
pub fn feature_filter<'g: 'a, 'a>(
    base: impl FeatureFilter<'g> + 'a,
    features: impl IntoIterator<Item = &'a str>,
) -> impl FeatureFilter<'g> + 'a {
    let mut base = base;
    let features: HashSet<_> = features.into_iter().collect();
    FeatureFilterFn::new(move |feature_graph, feature_id| {
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
pub fn feature_id_filter<'g: 'a, 'a>(
    base: impl FeatureFilter<'g> + 'a,
    feature_ids: impl IntoIterator<Item = impl Into<FeatureId<'a>>>,
) -> impl FeatureFilter<'g> + 'a {
    let mut base = base;
    let feature_ids: HashSet<_> = feature_ids
        .into_iter()
        .map(|feature_id| feature_id.into())
        .collect();
    FeatureFilterFn::new(move |feature_graph, feature_id| {
        base.accept(feature_graph, feature_id) || feature_ids.contains(&feature_id)
    })
}

/// A query over a feature graph.
///
/// This is the entry point for iterators overs IDs and dependency links, and dot graph presentation.
/// A `FeatureQuery` is constructed through the `query_` methods on `FeatureGraph`.
#[derive(Clone, Debug)]
pub struct FeatureQuery<'g> {
    graph: FeatureGraph<'g>,
    pub(super) params: QueryParams<FeatureGraph<'g>>,
}

/// ## Queries
///
/// The methods in this section create queries over subsets of this feature graph. Use the methods
/// here to analyze transitive dependencies.
impl<'g> FeatureGraph<'g> {
    /// Creates a new query over the entire workspace.
    ///
    /// `query_workspace` will select all workspace packages (subject to the provided filter) and
    /// their transitive dependencies.
    pub fn query_workspace(&self, filter: impl FeatureFilter<'g>) -> FeatureQuery<'g> {
        self.query_packages(&self.package_graph.query_workspace(), filter)
    }

    /// Creates a new query for all packages selected through this `PackageQuery` instance, subject
    /// to the provided filter.
    pub fn query_packages(
        &self,
        packages: &PackageQuery<'g>,
        filter: impl FeatureFilter<'g>,
    ) -> FeatureQuery<'g> {
        let params = match &packages.params {
            QueryParams::Forward(package_ixs) => QueryParams::Forward(
                self.feature_ixs_for_package_ixs_filtered(package_ixs.iter().copied(), filter),
            ),
            QueryParams::Reverse(package_ixs) => QueryParams::Reverse(
                self.feature_ixs_for_package_ixs_filtered(package_ixs.iter().copied(), filter),
            ),
        };

        FeatureQuery {
            graph: *self,
            params,
        }
    }

    /// Creates a new query that returns transitive dependencies of the given feature IDs in the
    /// specified direction.
    ///
    /// Returns an error if any feature IDs are unknown.
    pub fn query_directed<'a>(
        &self,
        feature_ids: impl IntoIterator<Item = impl Into<FeatureId<'a>>>,
        dep_direction: DependencyDirection,
    ) -> Result<FeatureQuery<'g>, Error> {
        match dep_direction {
            DependencyDirection::Forward => self.query_forward(feature_ids),
            DependencyDirection::Reverse => self.query_reverse(feature_ids),
        }
    }

    /// Creates a new query that returns transitive dependencies of the given feature IDs.
    ///
    /// Returns an error if any feature IDs are unknown.
    pub fn query_forward<'a>(
        &self,
        feature_ids: impl IntoIterator<Item = impl Into<FeatureId<'a>>>,
    ) -> Result<FeatureQuery<'g>, Error> {
        let feature_ids = feature_ids.into_iter().map(|feature_id| feature_id.into());
        Ok(FeatureQuery {
            graph: *self,
            params: QueryParams::Forward(self.feature_ixs(feature_ids)?),
        })
    }

    /// Creates a new query that returns transitive reverse dependencies of the given feature IDs.
    ///
    /// Returns an error if any feature IDs are unknown.
    pub fn query_reverse<'a>(
        &self,
        feature_ids: impl IntoIterator<Item = impl Into<FeatureId<'a>>>,
    ) -> Result<FeatureQuery<'g>, Error> {
        let feature_ids = feature_ids.into_iter().map(|feature_id| feature_id.into());
        Ok(FeatureQuery {
            graph: *self,
            params: QueryParams::Reverse(self.feature_ixs(feature_ids)?),
        })
    }
}

impl<'g> FeatureQuery<'g> {
    /// Resolves this selector into a set of known feature IDs.
    ///
    /// This is the entry point for iterators.
    pub fn resolve(self) -> FeatureSet<'g> {
        FeatureSet::new(self.graph, self.params)
    }
}
