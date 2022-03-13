// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::feature_helpers::assert_features_for_package;
use fixtures::{
    json::{self, JsonFixture},
    package_id,
};
use guppy::graph::{
    cargo::{CargoOptions, CargoResolverVersion, CargoSet},
    feature::{named_feature_filter, FeatureLabel, FeatureSet, StandardFeatures},
};

#[test]
fn default_features() {
    let cargo_set = make_cargo_set(feature_set_fn(&[]));
    assert_features_for_package(
        cargo_set.target_features(),
        &package_id(json::METADATA_WEAK_NAMESPACED_ID),
        Some(&[FeatureLabel::Base]),
        "while checking Cargo resolution for default features",
    );
}

#[test]
fn named_feature_single_dep() {
    let cargo_set = make_cargo_set(feature_set_fn(&["foo"]));

    // This should not the named feature foo + the optional dependency arrayvec (but not the
    // named feature arrayvec).
    assert_features_for_package(
        cargo_set.target_features(),
        &package_id(json::METADATA_WEAK_NAMESPACED_ID),
        Some(&[
            FeatureLabel::Base,
            FeatureLabel::Named("foo"),
            FeatureLabel::OptionalDependency("arrayvec"),
        ]),
        "while checking Cargo resolution for default + foo",
    );
}

#[test]
fn named_feature_same_as_dep_plus_feature() {
    let cargo_set = make_cargo_set(feature_set_fn(&["smallvec"]));

    // This should contain foo and both the named and optional dep versions of smallvec.
    assert_features_for_package(
        cargo_set.target_features(),
        &package_id(json::METADATA_WEAK_NAMESPACED_ID),
        Some(&[
            FeatureLabel::Base,
            FeatureLabel::Named("foo"),
            FeatureLabel::Named("smallvec"),
            FeatureLabel::OptionalDependency("arrayvec"),
            FeatureLabel::OptionalDependency("smallvec"),
        ]),
        "while checking Cargo resolution for default + smallvec",
    );
    // smallvec should not have its union feature enabled.
    assert_features_for_package(
        cargo_set.target_features(),
        &package_id(json::METADATA_WEAK_NAMESPACED_SMALLVEC),
        Some(&[FeatureLabel::Base]),
        "while checking Cargo resolution for default + smallvec",
    );
}

#[test]
fn enabled_non_weak_feature() {
    let cargo_set = make_cargo_set(feature_set_fn(&["bar"]));

    // This should enable both the feature and the optional dependency for camino.
    assert_features_for_package(
        cargo_set.target_features(),
        &package_id(json::METADATA_WEAK_NAMESPACED_ID),
        Some(&[
            FeatureLabel::Base,
            FeatureLabel::Named("arrayvec"),
            FeatureLabel::Named("bar"),
            FeatureLabel::OptionalDependency("arrayvec"),
        ]),
        "while checking Cargo resolution for default + bar",
    );
    assert_features_for_package(
        cargo_set.target_features(),
        &package_id(json::METADATA_WEAK_NAMESPACED_ARRAYVEC),
        Some(&[FeatureLabel::Base, FeatureLabel::Named("std")]),
        "while checking Cargo resolution for default + bar",
    );
}

#[test]
fn named_feature_does_not_enable_dep_with_same_name() {
    let cargo_set = make_cargo_set(feature_set_fn(&["arrayvec"]));

    // This should enable the named feature "arrayvec" but NOT the dependency arrayvec.
    assert_features_for_package(
        cargo_set.target_features(),
        &package_id(json::METADATA_WEAK_NAMESPACED_ID),
        Some(&[FeatureLabel::Base, FeatureLabel::Named("arrayvec")]),
        "while checking Cargo resolution for default + arrayvec",
    );
    assert_features_for_package(
        cargo_set.target_features(),
        &package_id(json::METADATA_WEAK_NAMESPACED_ARRAYVEC),
        None,
        "while checking Cargo resolution for default + arrayvec",
    );
}

#[test]
fn enabled_weak_feature_1() {
    let cargo_set = make_cargo_set(feature_set_fn(&["smallvec", "smallvec-union"]));

    // This should contain foo and both the named and optional dep versions of smallvec.
    assert_features_for_package(
        cargo_set.target_features(),
        &package_id(json::METADATA_WEAK_NAMESPACED_ID),
        Some(&[
            FeatureLabel::Base,
            FeatureLabel::Named("foo"),
            FeatureLabel::Named("smallvec"),
            FeatureLabel::Named("smallvec-union"),
            FeatureLabel::OptionalDependency("arrayvec"),
            FeatureLabel::OptionalDependency("smallvec"),
        ]),
        "while checking Cargo resolution for default + smallvec + smallvec-union",
    );
    // smallvec *should* have its union feature enabled.
    assert_features_for_package(
        cargo_set.target_features(),
        &package_id(json::METADATA_WEAK_NAMESPACED_SMALLVEC),
        Some(&[FeatureLabel::Base, FeatureLabel::Named("union")]),
        "while checking Cargo resolution for default + smallvec + smallvec-union",
    );
}

#[test]
fn enabled_weak_feature_2() {
    let cargo_set = make_cargo_set(feature_set_fn(&["foo", "baz"]));

    // This should enable the dependency arrayvec but NOT the named feature.
    assert_features_for_package(
        cargo_set.target_features(),
        &package_id(json::METADATA_WEAK_NAMESPACED_ID),
        Some(&[
            FeatureLabel::Base,
            FeatureLabel::Named("baz"),
            FeatureLabel::Named("foo"),
            FeatureLabel::OptionalDependency("arrayvec"),
            FeatureLabel::OptionalDependency("pathdiff"),
        ]),
        "while checking Cargo resolution for default + foo + baz",
    );
    assert_features_for_package(
        cargo_set.target_features(),
        &package_id(json::METADATA_WEAK_NAMESPACED_ARRAYVEC),
        Some(&[FeatureLabel::Base, FeatureLabel::Named("std")]),
        "while checking Cargo resolution for default + foo + baz",
    );
}

#[test]
fn enabled_weak_feature_3() {
    let cargo_set = make_cargo_set(feature_set_fn(&["bar", "baz"]));

    // This should enable BOTH the named feature and the dependency baz.
    assert_features_for_package(
        cargo_set.target_features(),
        &package_id(json::METADATA_WEAK_NAMESPACED_ID),
        Some(&[
            FeatureLabel::Base,
            FeatureLabel::Named("arrayvec"),
            FeatureLabel::Named("bar"),
            FeatureLabel::Named("baz"),
            FeatureLabel::OptionalDependency("arrayvec"),
            FeatureLabel::OptionalDependency("pathdiff"),
        ]),
        "while checking Cargo resolution for default + bar + baz",
    );
    assert_features_for_package(
        cargo_set.target_features(),
        &package_id(json::METADATA_WEAK_NAMESPACED_ARRAYVEC),
        Some(&[FeatureLabel::Base, FeatureLabel::Named("std")]),
        "while checking Cargo resolution for default + bar + baz",
    );
}

#[test]
fn disabled_weak_feature_1() {
    let cargo_set = make_cargo_set(feature_set_fn(&["baz"]));

    // This should NOT enable the dependency OR the named feature arrayvec.
    assert_features_for_package(
        cargo_set.target_features(),
        &package_id(json::METADATA_WEAK_NAMESPACED_ID),
        Some(&[
            FeatureLabel::Base,
            FeatureLabel::Named("baz"),
            // XXX: remove arrayvec once weak features are supported
            FeatureLabel::OptionalDependency("arrayvec"),
            FeatureLabel::OptionalDependency("pathdiff"),
        ]),
        "while checking Cargo resolution for default + baz",
    );
    assert_features_for_package(
        cargo_set.target_features(),
        &package_id(json::METADATA_WEAK_NAMESPACED_ARRAYVEC),
        // XXX: change this to None once weak features are supported
        Some(&[FeatureLabel::Base, FeatureLabel::Named("std")]),
        "while checking Cargo resolution for default + baz",
    );
}

#[test]
fn disabled_weak_feature_2() {
    let cargo_set = make_cargo_set(feature_set_fn(&["arrayvec", "baz"]));

    // This should enable the named feature arrayvec but NOT the dependency.
    assert_features_for_package(
        cargo_set.target_features(),
        &package_id(json::METADATA_WEAK_NAMESPACED_ID),
        Some(&[
            FeatureLabel::Base,
            FeatureLabel::Named("arrayvec"),
            FeatureLabel::Named("baz"),
            // XXX: remove arrayvec once weak features are supported
            FeatureLabel::OptionalDependency("arrayvec"),
            FeatureLabel::OptionalDependency("pathdiff"),
        ]),
        "while checking Cargo resolution for default + arrayvec + baz",
    );
    assert_features_for_package(
        cargo_set.target_features(),
        &package_id(json::METADATA_WEAK_NAMESPACED_ARRAYVEC),
        // XXX: change this to None once weak features are supported
        Some(&[FeatureLabel::Base, FeatureLabel::Named("std")]),
        "while checking Cargo resolution for default + arrayvec + baz",
    );
}

#[test]
fn check_feature_presence() {
    // Check the existence and non-existence of a few features.
    let feature_graph = JsonFixture::metadata_weak_namespaced_features()
        .graph()
        .feature_graph();

    assert!(feature_graph.contains((
        &package_id(json::METADATA_WEAK_NAMESPACED_ID),
        FeatureLabel::OptionalDependency("pathdiff")
    )));
    assert!(feature_graph.contains((
        &package_id(json::METADATA_WEAK_NAMESPACED_ID),
        FeatureLabel::Named("pathdiff2")
    )));
    assert!(!feature_graph.contains((
        &package_id(json::METADATA_WEAK_NAMESPACED_ID),
        FeatureLabel::Named("pathdiff")
    )));
    assert!(!feature_graph.contains((
        &package_id(json::METADATA_WEAK_NAMESPACED_ID),
        FeatureLabel::OptionalDependency("pathdiff2")
    )));
}

fn feature_set_fn(named_features: &[&str]) -> FeatureSet<'static> {
    JsonFixture::metadata_weak_namespaced_features()
        .graph()
        .resolve_ids([&package_id(json::METADATA_WEAK_NAMESPACED_ID)])
        .expect("valid package ID")
        .to_feature_set(named_feature_filter(
            StandardFeatures::Default,
            named_features.iter().copied(),
        ))
}

fn make_cargo_set(feature_set: FeatureSet<'static>) -> CargoSet<'static> {
    let mut cargo_options = CargoOptions::new();
    cargo_options.set_resolver(CargoResolverVersion::V2);

    feature_set
        .into_cargo_set(&cargo_options)
        .expect("resolving cargo should work")
}
