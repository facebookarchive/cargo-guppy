// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::iter::FromIterator;
use std::ops::Deref;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// A sorted, immutable vector.
pub struct SortedVec<T> {
    inner: Box<[T]>,
}

impl<T> SortedVec<T>
where
    T: Ord,
{
    /// Creates a new `SortedVec` from a vector or other slice container.
    pub fn new(v: impl Into<Box<[T]>>) -> Self {
        let mut v = v.into();
        v.sort();
        Self { inner: v }
    }

    // TODO: new + sort by/sort by key?

    /// Returns true if this sorted vector contains this element.
    pub fn contains(&self, item: &T) -> bool {
        self.binary_search(item).is_ok()
    }

    /// Returns the inner data.
    pub fn into_inner(self) -> Box<[T]> {
        self.inner
    }
}

impl<T> FromIterator<T> for SortedVec<T>
where
    T: Ord,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let v: Box<[T]> = iter.into_iter().collect();
        Self::new(v)
    }
}

impl<T> Deref for SortedVec<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
