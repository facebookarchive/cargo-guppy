// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Serialized versions of platform and target features.
//!
//! Some users of `target-spec` may want to serialize and deserialize its data structures into, say,
//! TOML files. This module provides facilities for that.
//!
//! Summaries require the `summaries` feature to be enabled.

use crate::{Error, Platform, TargetFeatures};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// An owned, serializable version of `Platform`.
///
/// This structure can be serialized and deserialized using `serde`.
///
/// Requires the `summaries` feature to be enabled.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct PlatformSummary {
    /// The platform triple.
    pub triple: String,

    /// The target features used.
    #[serde(with = "target_features_impl")]
    pub target_features: TargetFeaturesSummary,

    /// The flags enabled.
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub flags: BTreeSet<String>,
}

impl PlatformSummary {
    /// Creates a new `PlatformSummary` instance from a platform.
    ///
    /// Returns an error if this is a custom platform. Serializing custom platforms is currently
    /// unsupported.
    pub fn new(platform: &Platform<'_>) -> Result<Self, Error> {
        if platform.is_custom() {
            return Err(Error::CustomPlatformSummary);
        };
        Ok(Self {
            triple: platform.triple().to_string(),
            target_features: TargetFeaturesSummary::new(platform.target_features()),
            flags: platform.flags().map(|flag| flag.to_string()).collect(),
        })
    }

    /// Converts `self` to a `Platform`.
    ///
    /// Returns an `Error` if the platform was unknown.
    pub fn to_platform(&self) -> Result<Platform, Error> {
        let mut platform = Platform::new(&self.triple, self.target_features.to_target_features())?;
        platform.add_flags(self.flags.iter().map(|flag| flag.as_str()));
        Ok(platform)
    }
}

/// An owned, serializable version of `TargetFeatures`.
///
/// This type can be serialized and deserialized using `serde`.
///
/// Requires the `summaries` feature to be enabled.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", untagged)]
#[non_exhaustive]
pub enum TargetFeaturesSummary {
    /// The target features are unknown.
    Unknown,
    /// Only match the specified features.
    Features(BTreeSet<String>),
    /// Match all features.
    All,
}

impl TargetFeaturesSummary {
    /// Creates a new `TargetFeaturesSummary` from a `TargetFeatures`.
    pub fn new(target_features: &TargetFeatures<'_>) -> Self {
        match target_features {
            TargetFeatures::Unknown => TargetFeaturesSummary::Unknown,
            TargetFeatures::Features(features) => TargetFeaturesSummary::Features(
                features.iter().map(|feature| feature.to_string()).collect(),
            ),
            TargetFeatures::All => TargetFeaturesSummary::All,
        }
    }

    /// Converts `self` to a `TargetFeatures` instance.
    pub fn to_target_features(&self) -> TargetFeatures {
        match self {
            TargetFeaturesSummary::Unknown => TargetFeatures::Unknown,
            TargetFeaturesSummary::All => TargetFeatures::All,
            TargetFeaturesSummary::Features(features) => {
                let hash_set = features.iter().map(|feature| feature.as_str()).collect();
                TargetFeatures::Features(hash_set)
            }
        }
    }
}

mod target_features_impl {
    use super::*;
    use serde::{de::Error, Deserializer, Serializer};

    pub fn serialize<S>(
        target_features: &TargetFeaturesSummary,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match target_features {
            TargetFeaturesSummary::Unknown => "unknown".serialize(serializer),
            TargetFeaturesSummary::All => "all".serialize(serializer),
            TargetFeaturesSummary::Features(features) => features.serialize(serializer),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<TargetFeaturesSummary, D::Error>
    where
        D: Deserializer<'de>,
    {
        let d = TargetFeaturesDeserialize::deserialize(deserializer)?;
        match d {
            TargetFeaturesDeserialize::String(target_features) => match target_features.as_str() {
                "unknown" => Ok(TargetFeaturesSummary::Unknown),
                "all" => Ok(TargetFeaturesSummary::All),
                other => Err(D::Error::custom(format!(
                    "unknown string for target features: {}",
                    other,
                ))),
            },
            TargetFeaturesDeserialize::List(target_features) => {
                Ok(TargetFeaturesSummary::Features(target_features))
            }
        }
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum TargetFeaturesDeserialize {
        String(String),
        List(BTreeSet<String>),
    }
}

#[cfg(all(test, feature = "proptest010"))]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::HashSet;

    proptest! {
        #[test]
        fn summary_roundtrip(platform in Platform::strategy(any::<TargetFeatures<'static>>())) {
            let summary = PlatformSummary::new(&platform).expect("Platform::strategy does not generate custom platforms");
            let serialized = toml::ser::to_string(&summary).expect("serialization succeeded");

            let deserialized: PlatformSummary = toml::from_str(&serialized).expect("deserialization succeeded");
            assert_eq!(summary, deserialized, "summary and deserialized should match");
            let platform2 = deserialized.to_platform().expect("conversion to Platform succeeded");

            assert_eq!(platform.triple(), platform2.triple(), "triples match");
            assert_eq!(platform.target_features(), platform2.target_features(), "target features match");
            assert_eq!(platform.flags().collect::<HashSet<_>>(), platform2.flags().collect::<HashSet<_>>(), "flags match");
        }
    }
}
