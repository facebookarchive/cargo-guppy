// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::iter::FromIterator;
use std::ops::Deref;

/// An immutable set stored as a sorted vector.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SortedSet<T> {
    inner: Box<[T]>,
}

impl<T> SortedSet<T>
where
    T: Ord,
{
    /// Creates a new `SortedSet` from a vector or other slice container.
    pub fn new(v: impl Into<Vec<T>>) -> Self {
        let mut v = v.into();
        v.sort();
        v.dedup();
        Self { inner: v.into() }
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

impl<T> FromIterator<T> for SortedSet<T>
where
    T: Ord,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let v: Vec<T> = iter.into_iter().collect();
        Self::new(v)
    }
}

impl<T> Deref for SortedSet<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
