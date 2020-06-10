// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{Platform, TargetFeatures};
use cfg_expr::targets::ALL_BUILTINS;
use proptest::collection::hash_set;
use proptest::prelude::*;
use proptest::sample::select;

/// ## Helpers for property testing
///
/// The methods in this section allow `Platform` instances to be used in property-based testing
/// scenarios.
///
/// Currently, [proptest 0.10](https://docs.rs/proptest/0.10) is supported if the `proptest010`
/// feature is enabled.
impl<'a> Platform<'a> {
    /// Given a way to generate `TargetFeatures` instances, this returns a `Strategy` that generates
    /// a platform at random.
    ///
    /// Requires the `proptest010` feature to be enabled.
    ///
    /// ## Examples
    ///
    /// ```
    /// use proptest::prelude::*;
    /// use target_spec::{Platform, TargetFeatures};
    ///
    /// // target_features is a strategy that always produces TargetFeatures::Unknown.
    /// let target_features = Just(TargetFeatures::Unknown);
    /// let strategy = Platform::strategy(target_features);
    /// ```
    pub fn strategy(
        target_features: impl Strategy<Value = TargetFeatures<'a>> + 'a,
    ) -> impl Strategy<Value = Platform<'a>> + 'a {
        (0..ALL_BUILTINS.len(), target_features).prop_map(|(idx, target_features)| {
            Platform::new(ALL_BUILTINS[idx].triple, target_features).expect("known triple")
        })
    }

    /// A version of `strategy` that allows target triples to be filtered.
    ///
    /// Requires the `proptest010` feature to be enabled.
    pub fn filtered_strategy(
        triple_filter: impl Fn(&'static str) -> bool,
        target_features: impl Strategy<Value = TargetFeatures<'a>> + 'a,
    ) -> impl Strategy<Value = Platform<'a>> + 'a {
        let filtered: Vec<_> = ALL_BUILTINS
            .iter()
            .filter(|target_info| triple_filter(target_info.triple))
            .collect();
        (0..filtered.len(), target_features).prop_map(move |(idx, target_features)| {
            Platform::new(filtered[idx].triple, target_features).expect("known triple")
        })
    }
}

/// The `Arbitrary` implementation for `TargetFeatures` uses a predefined list of features.
impl Arbitrary for TargetFeatures<'static> {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        // https://doc.rust-lang.org/reference/attributes/codegen.html#available-features
        static KNOWN_FEATURES: &[&str] = &[
            "aes", "avx", "avx2", "bmi1", "bmi2", "fma", "rdrand", "sha", "sse", "sse2", "sse3",
            "sse4.1", "sse4.2", "ssse3", "xsave", "xsavec", "xsaveopt", "xsaves",
        ];
        prop_oneof![
            Just(TargetFeatures::Unknown),
            Just(TargetFeatures::All),
            hash_set(select(KNOWN_FEATURES), 0..8).prop_map(TargetFeatures::Features),
        ]
        .boxed()
    }
}
