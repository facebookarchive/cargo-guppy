// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;
use supercow::Supercow;

mod private {
    pub trait Sealed {}
}

/// Trait representing an owned, borrowed or shared instance of `T`.
///
/// This trait is implemented by `T`, `&'a T` and `Arc<T>`.
pub trait Obs<'a, T>: private::Sealed {
    #[doc(hidden)]
    fn into_supercow(self) -> Supercow<'a, T>;
}

impl<T> private::Sealed for T {}

impl<'a, T: Send> Obs<'a, T> for T {
    #[doc(hidden)]
    fn into_supercow(self) -> Supercow<'a, T> {
        self.into()
    }
}

impl<'a, T: Sync> Obs<'a, T> for &'a T {
    #[doc(hidden)]
    fn into_supercow(self) -> Supercow<'a, T> {
        self.into()
    }
}

impl<'a, T: 'static + Send + Sync> Obs<'a, T> for Arc<T> {
    #[doc(hidden)]
    fn into_supercow(self) -> Supercow<'a, T> {
        self.into()
    }
}
