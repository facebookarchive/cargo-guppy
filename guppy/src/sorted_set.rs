// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;
use std::iter::FromIterator;
use std::mem;
use std::ops::Deref;

/// A set stored as a sorted vector.
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

    /// Creates a new `SortedSet` with no contents.
    pub fn empty() -> Self {
        SortedSet {
            inner: Self::empty_box(),
        }
    }

    // TODO: new + sort by/sort by key?

    /// Returns true if this sorted vector contains this element.
    pub fn contains(&self, item: &T) -> bool {
        self.binary_search(item).is_ok()
    }

    /// Returns the data as a slice.
    pub fn as_slice(&self) -> &[T] {
        &self.inner
    }

    /// Adds the given elements to this vector.
    pub fn extend(&mut self, iter: impl Iterator<Item = T>) {
        let mut inner = mem::replace(&mut self.inner, Self::empty_box()).into_vec();
        inner.extend(iter);

        // Re-sort and dedup since there might be new repeated elements added.
        inner.sort();
        inner.dedup();

        mem::replace(&mut self.inner, inner.into());
    }

    /// Returns the inner data.
    pub fn into_inner(self) -> Box<[T]> {
        self.inner
    }

    // ---
    // Helper methods
    // ---

    fn empty_box() -> Box<[T]> {
        Box::new([])
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

impl fmt::Display for SortedSet<String> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{{}}}", self.as_slice().join(", "))
    }
}
