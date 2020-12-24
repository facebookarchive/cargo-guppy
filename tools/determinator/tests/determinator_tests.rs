// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Higher-level unit tests for the target determinator.

use determinator::{
    rules::{DeterminatorRules, PathMatch, RuleIndex},
    Determinator,
};
use fixtures::json::JsonFixture;
use guppy::graph::feature::StandardFeatures;
use std::path::Path;

#[test]
fn guppy_no_rules() {
    // There are no dependency changes between the old and new fixtures, only file changes.
    let old = JsonFixture::metadata_guppy_869476c();
    let new = JsonFixture::metadata_guppy_c9b4f76();

    let mut determinator = Determinator::new(old.graph(), new.graph());
    // Do not set custom rules -- ensure that default rules are used.

    // README.md is ignored by the default rules.
    determinator.add_changed_paths(vec![Path::new("README.md")]);
    let determinator_set = determinator.compute();
    assert!(
        determinator_set.path_changed_set.is_empty(),
        "nothing in workspace changed"
    );
    assert!(
        determinator_set.affected_set.is_empty(),
        "nothing in workspace affected"
    );

    // rust-toolchain causes a full build.
    determinator.add_changed_paths(vec![Path::new("rust-toolchain")]);
    let workspace_set = new.graph().resolve_workspace();
    let determinator_set = determinator.compute();
    assert_eq!(
        determinator_set.path_changed_set, workspace_set,
        "everything changed"
    );
    assert_eq!(
        determinator_set.affected_set, workspace_set,
        "everything changed"
    );
}
#[test]
fn guppy_path_rules() {
    // There are no dependency changes between the old and new fixtures, only file changes.
    let old = JsonFixture::metadata_guppy_869476c();
    let new = JsonFixture::metadata_guppy_c9b4f76();
    let opts = read_options(new, "path-rules.toml");

    let mut determinator = Determinator::new(old.graph(), new.graph());
    determinator.set_rules(&opts).expect("rules set correctly");

    let determinator_set = determinator.compute();
    assert!(
        determinator_set.path_changed_set.is_empty(),
        "nothing in workspace changed"
    );
    assert!(
        determinator_set.affected_set.is_empty(),
        "nothing in workspace affected"
    );

    // Try adding some files -- this isn't matched by any rule.
    determinator.add_changed_paths(vec![Path::new("fixtures/src/details.rs")]);
    let expected_changed = new
        .graph()
        .resolve_workspace_names(vec!["fixtures"])
        .expect("workspace names resolved");
    let expected_affected = new
        .graph()
        .resolve_workspace_names(vec![
            // fixtures is a test-only dependency of guppy, so guppy's transitive dependencies
            // aren't involved. fixture-manager is not depended on by anyone.
            "fixtures",
            "guppy",
            "fixture-manager",
        ])
        .expect("workspace names resolved");

    {
        let determinator_set = determinator.compute();
        assert_eq!(determinator_set.path_changed_set, expected_changed);
        assert_eq!(determinator_set.affected_set, expected_affected);
    }

    // Add a README, which is ignored by the rules.
    determinator.add_changed_paths(vec![
        Path::new("guppy/README.md"),
        Path::new("cargo-guppy/README.tpl"),
    ]);
    {
        let determinator_set = determinator.compute();
        assert_eq!(determinator_set.path_changed_set, expected_changed);
        assert_eq!(determinator_set.affected_set, expected_affected);
    }

    // Cargo.lock and .gitignore should be ignored by default and shouldn't cause any changes.
    determinator.add_changed_paths(vec![Path::new(".gitignore"), Path::new("Cargo.lock")]);
    {
        let determinator_set = determinator.compute();
        assert_eq!(determinator_set.path_changed_set, expected_changed);
        assert_eq!(determinator_set.affected_set, expected_affected);
    }

    // Check that rules doesn't apply to subdirectories.
    determinator.add_changed_paths(vec![Path::new("foo/CODE_OF_CONDUCT.md")]);
    {
        let determinator_set = determinator.compute();
        assert_eq!(determinator_set.path_changed_set, expected_changed);
        assert_eq!(determinator_set.affected_set, expected_affected);
    }

    // Ensure that fallthrough works.

    determinator.add_changed_paths(vec![Path::new("CONTRIBUTING.md")]);
    {
        // CONTRIBUTING.md should cause cargo-guppy to be added.
        let new_changed = new
            .graph()
            .resolve_workspace_names(vec!["cargo-guppy", "fixtures"])
            .expect("workspace names resolved");
        let new_affected = expected_affected.union(
            &new.graph()
                .resolve_workspace_names(vec!["cargo-guppy"])
                .expect("workspace names resolved"),
        );
        let determinator_set = determinator.compute();
        assert_eq!(determinator_set.path_changed_set, new_changed);
        assert_eq!(determinator_set.affected_set, new_affected);
    }
    determinator.add_changed_paths(vec![Path::new("CODE_OF_CONDUCT.md")]);
    {
        // CODE_OF_CONDUCT.md should cause both guppy and cargo-guppy to be added.
        let new_changed = new
            .graph()
            .resolve_workspace_names(vec!["cargo-guppy", "fixtures", "guppy"])
            .expect("workspace names resolved");
        let new_affected = new_changed.union(
            &new.graph()
                .resolve_workspace_names(
                    // These are all packages that guppy is a dependency of.
                    vec![
                        "guppy-cmdlib",
                        "guppy-benchmarks",
                        "cargo-compare",
                        "fixture-manager",
                    ],
                )
                .expect("workspace names resolved"),
        );
        let determinator_set = determinator.compute();
        assert_eq!(determinator_set.path_changed_set, new_changed);
        assert_eq!(determinator_set.affected_set, new_affected);
    }

    // Ensure that skip-rules works as expected, skipping further rules.
    determinator.add_changed_paths(vec![Path::new("internal-tools/benchmarks/foo")]);
    {
        // CODE_OF_CONDUCT.md should cause both guppy and cargo-guppy to be added.
        let new_changed = new
            .graph()
            .resolve_workspace_names(vec![
                "cargo-guppy",
                "fixtures",
                "guppy",
                "guppy-benchmarks",
                "cargo-compare",
            ])
            .expect("workspace paths resolved");
        let new_affected = new_changed.union(
            &new.graph()
                .resolve_workspace_names(
                    // These are all packages that guppy is a dependency of.
                    vec!["guppy-cmdlib", "fixture-manager"],
                )
                .expect("workspace names resolved"),
        );

        let determinator_set = determinator.compute();
        assert_eq!(determinator_set.path_changed_set, new_changed);
        assert_eq!(determinator_set.affected_set, new_affected);
    }
}

#[test]
fn guppy_package_rules() {
    // There are no dependency changes between the old and new fixtures, only file changes.
    let old = JsonFixture::metadata_guppy_869476c();
    let new = JsonFixture::metadata_guppy_c9b4f76();
    let opts = read_options(new, "package-rules.toml");

    let mut determinator = Determinator::new(old.graph(), new.graph());
    determinator.set_rules(&opts).expect("rules set correctly");

    // Nothing changed means empty set.
    let determinator_set = determinator.compute();
    assert!(
        determinator_set.path_changed_set.is_empty(),
        "no path changes"
    );
    assert!(
        determinator_set.summary_changed_set.is_empty(),
        "no summary changes"
    );
    assert!(determinator_set.affected_set.is_empty(), "no changes");

    {
        // This ruleset disables default rules, so Cargo.lock changing should cause everything to be
        // built.
        let mut determinator = determinator.clone();
        determinator.add_changed_paths(vec![Path::new("Cargo.lock")]);
        let determinator_set = determinator.compute();
        let workspace_set = new.graph().resolve_workspace();
        assert_eq!(
            determinator_set.path_changed_set, workspace_set,
            "everything changed"
        );
        assert_eq!(
            determinator_set.affected_set, workspace_set,
            "everything changed"
        );
    }

    // Add a file that doesn't match any of the rules.
    determinator.add_changed_paths(vec![Path::new("cargo-guppy/foo.rs")]);
    let determinator_set = determinator.compute();
    let expected_path_changed = new
        .graph()
        .resolve_workspace_names(vec!["cargo-guppy"])
        .expect("valid workspace names");

    assert_eq!(
        determinator_set.path_changed_set, expected_path_changed,
        "cargo-guppy in path changes"
    );
    assert!(
        determinator_set.summary_changed_set.is_empty(),
        "no summary changes"
    );
    assert_eq!(
        determinator_set.affected_set, expected_path_changed,
        "cargo-guppy in affected set"
    );

    // Add a file which matches fixtures (and triggers guppy-cmdlib).
    determinator.add_changed_paths(vec![Path::new("fixtures/src/main.rs")]);
    let determinator_set = determinator.compute();
    let expected_path_changed = new
        .graph()
        .resolve_workspace_names(vec!["cargo-guppy", "fixtures"])
        .expect("valid workspace names");
    let expected_affected = expected_path_changed.union(
        &new.graph()
            .resolve_workspace_names(vec![
                // fixtures is a *dev dependency* of guppy, so guppy itself is affected but
                // packages that depend on it, such as guppy-benchmarks, are *not*.
                "guppy",
                // guppy-cmdlib is added through a package rule.
                "guppy-cmdlib",
                // cargo-compare depends on guppy-cmdlib.
                "cargo-compare",
                // fixture-manager depends on guppy-cmdlib.
                "fixture-manager",
            ])
            .expect("valid workspace names"),
    );

    assert_eq!(
        determinator_set.path_changed_set, expected_path_changed,
        "cargo-guppy + fixtures in path changes"
    );
    assert!(
        determinator_set.summary_changed_set.is_empty(),
        "no summary changes"
    );
    assert_eq!(
        determinator_set.affected_set, expected_affected,
        "most but not all packages affected"
    );
}

#[test]
fn guppy_package_rules_2() {
    // There are no dependency changes between the old and new fixtures, only file changes.
    let old = JsonFixture::metadata_guppy_869476c();
    let new = JsonFixture::metadata_guppy_c9b4f76();
    let opts = read_options(new, "package-rules.toml");

    let mut determinator = Determinator::new(old.graph(), new.graph());
    determinator.set_rules(&opts).expect("rules set correctly");

    // Changing a "fake-trigger" file means "proptest-ext" changes, which causes "guppy-benchmarks"
    // to change, which according to a package rule means everything gets rebuilt.
    determinator.add_changed_paths(vec![Path::new("foo/fake-trigger")]);
    let determinator_set = determinator.compute();
    let expected_path_changed = new
        .graph()
        .resolve_workspace_names(vec!["proptest-ext"])
        .expect("valid workspace names");

    assert_eq!(
        determinator_set.path_changed_set, expected_path_changed,
        "cargo-guppy + fixtures in path changes"
    );
    assert!(
        determinator_set.summary_changed_set.is_empty(),
        "no summary changes"
    );
    assert_eq!(
        determinator_set.affected_set,
        new.graph().resolve_workspace(),
        "all packages affected"
    );
}

#[test]
fn guppy_deps() {
    // new updates the version of toml, which should cause most things to change.
    let old = JsonFixture::metadata_guppy_78cb7e8();
    let new = JsonFixture::metadata_guppy_869476c();
    let opts = read_options(new, "path-rules.toml");

    let mut determinator = Determinator::new(old.graph(), new.graph());
    determinator.set_rules(&opts).expect("rules set correctly");

    // This changes the "toml" dependency, so many packages should be marked changed.
    let determinator_set = determinator.compute();
    let expected = new
        .graph()
        .resolve_workspace_names(vec![
            "cargo-guppy",
            "fixture-manager",
            "guppy",
            "guppy-summaries",
            "cargo-compare",
            // toml is only a dev-dependency for target-spec. Ensure that it's marked as changed.
            "target-spec",
            // Packages not marked changed include "fixtures", "guppy-cmdlib" and
            // "guppy-benchmarks". While these packages depend on guppy, they don't enable the
            // summaries feature so they aren't influenced by the toml dependency.
        ])
        .expect("workspace names resolved");

    assert!(
        determinator_set.path_changed_set.is_empty(),
        "no path changes"
    );
    assert_eq!(
        determinator_set.summary_changed_set, expected,
        "some summary changes"
    );
    assert_eq!(
        determinator_set.affected_set, expected,
        "some packages affected"
    );

    // Try setting fixture-manager as features-only. This should cause guppy's summaries feature to
    // always be enabled, which means that fixtures, guppy-cmdlib and guppy-benchmarks should be
    // added to the expected set.
    determinator
        .set_features_only(
            ["fixture-manager"].iter().copied(),
            StandardFeatures::Default,
        )
        .expect("fixture-manager is a valid package name");

    let determinator_set = determinator.compute();
    let features_only_expected = expected.union(
        &new.graph()
            .resolve_workspace_names(vec!["fixtures", "guppy-cmdlib", "guppy-benchmarks"])
            .expect("workspace names resolved"),
    );
    assert!(
        determinator_set.path_changed_set.is_empty(),
        "no path changes"
    );
    assert_eq!(
        determinator_set.summary_changed_set, features_only_expected,
        "some summary changes"
    );
    assert_eq!(
        determinator_set.affected_set, features_only_expected,
        "some packages affected"
    );
}

#[test]
fn guppy_match_paths() {
    let old = JsonFixture::metadata_guppy_869476c();
    let new = JsonFixture::metadata_guppy_c9b4f76();
    let opts = read_options(new, "path-rules.toml");

    let mut determinator = Determinator::new(old.graph(), new.graph());
    determinator
        .set_rules(&opts)
        .expect("options set correctly");

    // These expected outputs were figured out by manually matching path-rules.toml and
    // default-rules.toml.
    let expected = vec![
        ("Cargo.toml", PathMatch::RuleMatchedAll),
        (
            "README.md",
            PathMatch::RuleMatched(RuleIndex::CustomPath(0)),
        ),
        (
            "foo/README.tpl",
            PathMatch::RuleMatched(RuleIndex::CustomPath(0)),
        ),
        (
            "CONTRIBUTING.md",
            PathMatch::RuleMatched(RuleIndex::DefaultPath(4)),
        ),
        (
            "CODE_OF_CONDUCT.md",
            PathMatch::RuleMatched(RuleIndex::CustomPath(2)),
        ),
        ("guppy/src/foo", PathMatch::AncestorMatched),
        ("guppy/src/lib.rs", PathMatch::AncestorMatched),
        (
            "Cargo.lock",
            PathMatch::RuleMatched(RuleIndex::DefaultPath(3)),
        ),
    ];

    for (path, m) in expected {
        assert_eq!(
            determinator.match_path(path, |_| {}),
            m,
            "expected rule match for {}",
            path
        );
    }
}

fn read_options(fixture: &JsonFixture, toml_name: &str) -> DeterminatorRules {
    // Path to the determinator.toml file.
    let mut toml_path = fixture.abs_path().to_path_buf();
    toml_path.pop();
    toml_path.push(toml_name);

    let opts =
        std::fs::read_to_string(&toml_path).expect("determinator.toml was successfully read");
    DeterminatorRules::parse(&opts).expect("determinator.toml parsed")
}
