// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::{ops::Deref, sync::Arc};
use supercow::Supercow;

/// Represents an owned, borrowed or shared instance of `T`.
///
/// This represents any of `T`, `&'a T` or `Arc<T>`.
#[derive(Clone, Debug)]
pub struct Obs<'a, T>(Supercow<'a, T>);

impl<'a, T> Deref for Obs<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<'a, T: Send> From<T> for Obs<'a, T> {
    fn from(t: T) -> Obs<'a, T> {
        Obs(t.into())
    }
}

impl<'a, T: Sync> From<&'a T> for Obs<'a, T> {
    fn from(t: &'a T) -> Obs<'a, T> {
        Obs(t.into())
    }
}

impl<'a, T: 'static + Send + Sync> From<Arc<T>> for Obs<'a, T> {
    fn from(t: Arc<T>) -> Obs<'a, T> {
        Obs(t.into())
    }
}
