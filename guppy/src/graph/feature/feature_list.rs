// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A sorted, deduplicated list of features from a single package.

use crate::{
    graph::{feature::FeatureId, PackageMetadata},
    sorted_set::SortedSet,
    PackageId,
};
use std::{fmt, slice, vec};

/// A sorted, deduplicated list of features from a single package.
///
/// This provides a convenient way to query and print out lists of features.
///
/// Returned by methods on `FeatureSet`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeatureList<'g> {
    package: PackageMetadata<'g>,
    features: SortedSet<&'g str>,
    has_base: bool,
}

impl<'g> FeatureList<'g> {
    /// Creates a new `FeatureList` from a package and an iterator over features.
    pub fn new(
        package: PackageMetadata<'g>,
        features: impl IntoIterator<Item = Option<&'g str>>,
    ) -> Self {
        let mut has_base = false;
        let features = features
            .into_iter()
            .filter_map(|feature| match feature {
                Some(feature) => Some(feature),
                None => {
                    has_base = true;
                    None
                }
            })
            .collect();
        Self {
            package,
            features,
            has_base,
        }
    }

    /// Returns the package corresponding to this feature list.
    pub fn package(&self) -> &PackageMetadata<'g> {
        &self.package
    }

    /// Returns true if this feature list contains this feature.
    pub fn contains(&self, feature: impl AsRef<str>) -> bool {
        self.features.contains(&feature.as_ref())
    }

    /// Returns true if this feature list contains the "base" feature.
    ///
    /// The "base" feature represents the package with no features enabled.
    pub fn has_base(&self) -> bool {
        self.has_base
    }

    /// Returns the list of features as a slice.
    ///
    /// This slice is guaranteed to be sorted and unique.
    pub fn features(&self) -> &[&'g str] {
        self.features.as_slice()
    }

    /// Returns a borrowed iterator over feature IDs.
    pub fn iter<'a>(&'a self) -> Iter<'g, 'a> {
        self.into_iter()
    }

    /// Returns a pretty-printer over the list of features.
    pub fn display_features<'a>(&'a self) -> DisplayFeatures<'g, 'a> {
        DisplayFeatures(self.features())
    }

    /// Returns a vector of feature names.
    ///
    /// The vector is guaranteed to be sorted and unique.
    pub fn into_features(self) -> Vec<&'g str> {
        self.features.into_inner().into_vec()
    }
}

impl<'g> IntoIterator for FeatureList<'g> {
    type Item = FeatureId<'g>;
    type IntoIter = IntoIter<'g>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter::new(self)
    }
}

impl<'a, 'g> IntoIterator for &'a FeatureList<'g> {
    type Item = FeatureId<'g>;
    type IntoIter = Iter<'g, 'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter::new(self)
    }
}

/// An owned iterator over a `FeatureList`.
pub struct IntoIter<'g> {
    package_id: &'g PackageId,
    has_base: bool,
    iter: vec::IntoIter<&'g str>,
}

impl<'g> IntoIter<'g> {
    /// Creates a new iterator.
    pub fn new(feature_list: FeatureList<'g>) -> Self {
        Self {
            package_id: feature_list.package.id(),
            has_base: feature_list.has_base,
            iter: feature_list.into_features().into_iter(),
        }
    }
}

impl<'g> Iterator for IntoIter<'g> {
    type Item = FeatureId<'g>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.has_base {
            self.has_base = false;
            return Some(FeatureId::base(self.package_id));
        }
        self.iter
            .next()
            .map(|feature| FeatureId::new(self.package_id, feature))
    }
}

/// A borrowed iterator over a `FeatureList`.
pub struct Iter<'g, 'a> {
    package_id: &'g PackageId,
    has_base: bool,
    iter: slice::Iter<'a, &'g str>,
}

impl<'g, 'a> Iter<'g, 'a> {
    /// Creates a new iterator.
    pub fn new(feature_list: &'a FeatureList<'g>) -> Self {
        Self {
            package_id: feature_list.package.id(),
            has_base: feature_list.has_base,
            iter: feature_list.features().iter(),
        }
    }
}

impl<'g, 'a> Iterator for Iter<'g, 'a> {
    type Item = FeatureId<'g>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.has_base {
            self.has_base = false;
            return Some(FeatureId::base(self.package_id));
        }
        self.iter
            .next()
            .map(|feature| FeatureId::new(self.package_id, feature))
    }
}

/// A pretty-printer for a list of features.
///
/// Returned by `FeatureList::display_filters`.
#[derive(Clone, Copy, Debug)]
pub struct DisplayFeatures<'g, 'a>(&'a [&'g str]);

impl<'g, 'a> fmt::Display for DisplayFeatures<'g, 'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.join(", "))
    }
}
