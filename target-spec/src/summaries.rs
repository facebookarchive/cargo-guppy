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
use std::{borrow::Cow, collections::BTreeSet};

impl Platform {
    /// Converts this `Platform` to a serializable form.
    ///
    /// Requires the `summaries` feature to be enabled.
    #[inline]
    pub fn to_summary(&self) -> PlatformSummary {
        PlatformSummary::new(self)
    }
}

/// An owned, serializable version of `Platform`.
///
/// This structure can be serialized and deserialized using `serde`.
///
/// Requires the `summaries` feature to be enabled.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct PlatformSummary {
    /// The platform triple.
    pub triple: String,

    /// The target features used.
    pub target_features: TargetFeaturesSummary,

    /// The flags enabled.
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub flags: BTreeSet<String>,
}

impl PlatformSummary {
    /// Creates a new `PlatformSummary` instance from a platform.
    pub fn new(platform: &Platform) -> Self {
        Self {
            triple: platform.triple_str().to_string(),
            target_features: TargetFeaturesSummary::new(platform.target_features()),
            flags: platform.flags().map(|flag| flag.to_string()).collect(),
        }
    }

    /// Converts `self` to a `Platform`.
    ///
    /// Returns an `Error` if the platform was unknown.
    pub fn to_platform(&self) -> Result<Platform, Error> {
        let mut platform = Platform::new(
            self.triple.to_owned(),
            self.target_features.to_target_features(),
        )?;
        platform.add_flags(self.flags.iter().cloned());
        Ok(platform)
    }
}

/// An owned, serializable version of `TargetFeatures`.
///
/// This type can be serialized and deserialized using `serde`.
///
/// Requires the `summaries` feature to be enabled.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TargetFeaturesSummary {
    /// The target features are unknown.
    ///
    /// This is the default.
    Unknown,
    /// Only match the specified features.
    Features(BTreeSet<String>),
    /// Match all features.
    All,
}

impl TargetFeaturesSummary {
    /// Creates a new `TargetFeaturesSummary` from a `TargetFeatures`.
    pub fn new(target_features: &TargetFeatures) -> Self {
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
                let features = features
                    .iter()
                    .map(|feature| Cow::Owned(feature.clone()))
                    .collect();
                TargetFeatures::Features(features)
            }
        }
    }
}

impl Default for TargetFeaturesSummary {
    #[inline]
    fn default() -> Self {
        TargetFeaturesSummary::Unknown
    }
}

mod platform_impl {
    use super::*;
    use serde::Deserializer;

    impl<'de> Deserialize<'de> for PlatformSummary {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let d = PlatformSummaryDeserialize::deserialize(deserializer)?;
            match d {
                PlatformSummaryDeserialize::String(triple) => Ok(PlatformSummary {
                    triple,
                    target_features: TargetFeaturesSummary::default(),
                    flags: BTreeSet::default(),
                }),
                PlatformSummaryDeserialize::Full {
                    triple,
                    target_features,
                    flags,
                } => Ok(PlatformSummary {
                    triple,
                    target_features,
                    flags,
                }),
            }
        }
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum PlatformSummaryDeserialize {
        String(String),
        #[serde(rename_all = "kebab-case")]
        Full {
            triple: String,
            /// The target features used.
            #[serde(default)]
            target_features: TargetFeaturesSummary,
            /// The flags enabled.
            #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
            flags: BTreeSet<String>,
        },
    }
}

mod target_features_impl {
    use super::*;
    use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};

    impl Serialize for TargetFeaturesSummary {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            match self {
                TargetFeaturesSummary::Unknown => "unknown".serialize(serializer),
                TargetFeaturesSummary::All => "all".serialize(serializer),
                TargetFeaturesSummary::Features(features) => features.serialize(serializer),
            }
        }
    }

    impl<'de> Deserialize<'de> for TargetFeaturesSummary {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let d = TargetFeaturesDeserialize::deserialize(deserializer)?;
            match d {
                TargetFeaturesDeserialize::String(target_features) => {
                    match target_features.as_str() {
                        "unknown" => Ok(TargetFeaturesSummary::Unknown),
                        "all" => Ok(TargetFeaturesSummary::All),
                        other => Err(D::Error::custom(format!(
                            "unknown string for target features: {}",
                            other,
                        ))),
                    }
                }
                TargetFeaturesDeserialize::List(target_features) => {
                    Ok(TargetFeaturesSummary::Features(target_features))
                }
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

#[cfg(test)]
mod tests {
    #![allow(clippy::vec_init_then_push)]

    use super::*;

    #[test]
    fn platform_deserialize_valid() {
        // Need a wrapper because of TOML restrictions
        #[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
        struct Wrapper {
            platform: PlatformSummary,
        }

        let mut valid = vec![];
        valid.push((
            r#"platform = "x86_64-unknown-linux-gnu""#,
            PlatformSummary {
                triple: "x86_64-unknown-linux-gnu".into(),
                target_features: TargetFeaturesSummary::Unknown,
                flags: BTreeSet::new(),
            },
        ));
        valid.push((
            r#"platform = { triple = "x86_64-unknown-linux-gnu" }"#,
            PlatformSummary {
                triple: "x86_64-unknown-linux-gnu".into(),
                target_features: TargetFeaturesSummary::Unknown,
                flags: BTreeSet::new(),
            },
        ));
        valid.push((
            r#"platform = { triple = "x86_64-unknown-linux-gnu", target-features = "unknown" }"#,
            PlatformSummary {
                triple: "x86_64-unknown-linux-gnu".into(),
                target_features: TargetFeaturesSummary::Unknown,
                flags: BTreeSet::new(),
            },
        ));
        valid.push((
            r#"platform = { triple = "x86_64-unknown-linux-gnu", target-features = "all" }"#,
            PlatformSummary {
                triple: "x86_64-unknown-linux-gnu".into(),
                target_features: TargetFeaturesSummary::All,
                flags: BTreeSet::new(),
            },
        ));
        valid.push((
            r#"platform = { triple = "x86_64-unknown-linux-gnu", target-features = [] }"#,
            PlatformSummary {
                triple: "x86_64-unknown-linux-gnu".into(),
                target_features: TargetFeaturesSummary::Features(BTreeSet::new()),
                flags: BTreeSet::new(),
            },
        ));

        let mut flags = BTreeSet::new();
        flags.insert("cargo_web".to_owned());
        valid.push((
            r#"platform = { triple = "x86_64-unknown-linux-gnu", flags = ["cargo_web"] }"#,
            PlatformSummary {
                triple: "x86_64-unknown-linux-gnu".into(),
                target_features: TargetFeaturesSummary::Unknown,
                flags,
            },
        ));

        for (input, expected) in valid {
            let actual: Wrapper = toml::from_str(input)
                .unwrap_or_else(|err| panic!("input {} is valid: {}", input, err));
            assert_eq!(actual.platform, expected, "for input: {}", input);

            // Serialize and deserialize again.
            let serialized = toml::to_string(&actual).expect("serialized correctly");
            let actual_2: Wrapper = toml::from_str(&serialized)
                .unwrap_or_else(|err| panic!("serialized input: {} is valid: {}", input, err));
            assert_eq!(actual, actual_2, "for input: {}", input);
        }
    }
}

#[cfg(all(test, feature = "proptest1"))]
mod proptests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::HashSet;

    proptest! {
        #[test]
        fn summary_roundtrip(platform in Platform::strategy(any::<TargetFeatures>())) {
            let summary = PlatformSummary::new(&platform);
            let serialized = toml::ser::to_string(&summary).expect("serialization succeeded");

            let deserialized: PlatformSummary = toml::from_str(&serialized).expect("deserialization succeeded");
            assert_eq!(summary, deserialized, "summary and deserialized should match");
            let platform2 = deserialized.to_platform().expect("conversion to Platform succeeded");

            assert_eq!(platform.triple_str(), platform2.triple_str(), "triples match");
            assert_eq!(platform.target_features(), platform2.target_features(), "target features match");
            assert_eq!(platform.flags().collect::<HashSet<_>>(), platform2.flags().collect::<HashSet<_>>(), "flags match");
        }
    }
}
