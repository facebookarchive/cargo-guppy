// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{
    diff::SummaryDiffStatus, PackageInfo, PackageMap, PackageStatus, SummaryId, SummarySource,
    SummaryWithMetadata,
};
use pretty_assertions::assert_eq;
use semver::Version;
use std::collections::BTreeSet;

type Summary = SummaryWithMetadata;

static SERIALIZED_SUMMARY: &str = r#"# This is a test @generated summary.

[[target-package]]
name = 'foo'
version = '1.2.3'
workspace-path = 'foo'
status = 'initial'
features = ['default', 'feature1']

[[target-package]]
name = 'dep'
version = '0.4.2'
crates-io = true
status = 'direct'
features = ['std']

[[target-package]]
name = 'no-changes'
version = '1.5.3'
crates-io = true
status = 'transitive'
features = ['default']

[[host-package]]
name = 'bar'
version = '0.1.0'
workspace-path = 'dir/bar'
status = 'workspace'
features = ['default', 'feature2']

[[host-package]]
name = 'local-dep'
version = '1.1.2'
path = '../local-dep'
status = 'transitive'
features = []
"#;

static SUMMARY2: &str = r#"# This is a test @generated summary.

[[target-package]]
name = 'foo'
version = '1.2.3'
workspace-path = 'foo'
status = 'initial'
features = ['default', 'feature1', 'feature2']

[[target-package]]
name = 'dep'
version = '0.4.3'
crates-io = true
status = 'direct'
features = ['std']

[[target-package]]
name = 'dep'
version = '0.5.0'
crates-io = true
status = 'transitive'
features = ['std']

[[target-package]]
name = 'no-changes'
version = '1.5.3'
crates-io = true
status = 'transitive'
features = ['default']

[[host-package]]
name = 'bar'
version = '0.2.0'
workspace-path = 'dir/bar'
status = 'initial'
features = ['default', 'feature2']

[[host-package]]
name = 'local-dep'
version = '1.1.2'
path = '../local-dep'
status = 'transitive'
features = ['dep-feature']

[[host-package]]
name = 'local-dep'
version = '2.0.0'
path = '../local-dep-2'
status = 'transitive'
features = []
"#;

#[test]
fn empty_roundtrip() {
    let summary = Summary::default();

    let mut s = "# This is a test @generated summary.\n\n".to_string();
    summary.write_to_string(&mut s).expect("write succeeded");

    static SERIALIZED_SUMMARY: &str = "# This is a test @generated summary.\n\n";

    assert_eq!(&s, SERIALIZED_SUMMARY, "serialized representation matches");

    let deserialized = Summary::parse(&s).expect("from_str succeeded");
    assert_eq!(summary, deserialized, "deserialized representation matches");

    let diff = summary.diff(&deserialized);
    assert!(diff.is_unchanged(), "diff should be empty");
}

#[test]
fn basic_roundtrip() {
    let target_packages = vec![
        (
            SummaryId::new(
                "foo",
                Version::new(1, 2, 3),
                SummarySource::workspace("foo"),
            ),
            PackageStatus::Initial,
            vec!["default", "feature1"],
        ),
        (
            SummaryId::new("dep", Version::new(0, 4, 2), SummarySource::crates_io()),
            PackageStatus::Direct,
            vec!["std"],
        ),
        (
            SummaryId::new(
                "no-changes",
                Version::new(1, 5, 3),
                SummarySource::crates_io(),
            ),
            PackageStatus::Transitive,
            vec!["default"],
        ),
    ];
    let host_packages = vec![
        (
            SummaryId::new(
                "bar",
                Version::new(0, 1, 0),
                SummarySource::workspace("dir/bar"),
            ),
            PackageStatus::Workspace,
            vec!["default", "feature2"],
        ),
        (
            SummaryId::new(
                "local-dep",
                Version::new(1, 1, 2),
                SummarySource::path("../local-dep"),
            ),
            PackageStatus::Transitive,
            vec![],
        ),
    ];

    let summary = Summary {
        metadata: None,
        target_packages: make_summary(target_packages),
        host_packages: make_summary(host_packages),
    };

    let mut s = "# This is a test @generated summary.\n\n".to_string();
    summary.write_to_string(&mut s).expect("write succeeded");

    assert_eq!(&s, SERIALIZED_SUMMARY, "serialized representation matches");

    let deserialized = Summary::parse(&s).expect("from_str succeeded");
    assert_eq!(summary, deserialized, "deserialized representation matches");

    let diff = summary.diff(&deserialized);
    assert!(diff.is_unchanged(), "diff should be empty");

    // Try changing some things.
    let summary2 = Summary::parse(SUMMARY2).expect("from_str succeeded");
    let diff = summary.diff(&summary2);

    // target_packages is:
    // * a change for foo = 1 entry
    // * a remove + 2 inserts for dep (so it should not be combined) = 3 entries
    assert_eq!(diff.target_packages.changed.len(), 4, "4 changed entries");
    let mut iter = diff.target_packages.changed.iter();

    // First, dep 0.4.2.
    let std_feature: BTreeSet<_> = vec!["std".to_string()].into_iter().collect();
    let (summary_id, status) = iter.next().expect("3 elements left");
    assert_eq!(summary_id.name, "dep");
    assert_eq!(summary_id.version.to_string(), "0.4.2");
    assert_eq!(summary_id.source, SummarySource::crates_io());
    assert_eq!(
        *status,
        SummaryDiffStatus::Removed {
            old_info: &PackageInfo {
                status: PackageStatus::Direct,
                features: std_feature.clone(),
            },
        },
    );

    // Next, dep 0.4.3.
    let (summary_id, status) = iter.next().expect("2 elements left");
    assert_eq!(summary_id.name, "dep");
    assert_eq!(summary_id.version.to_string(), "0.4.3");
    assert_eq!(summary_id.source, SummarySource::crates_io());
    assert_eq!(
        *status,
        SummaryDiffStatus::Added {
            info: &PackageInfo {
                status: PackageStatus::Direct,
                features: std_feature.clone(),
            },
        },
    );

    // Next, dep 0.5.0.
    let (summary_id, status) = iter.next().expect("1 element left");
    assert_eq!(summary_id.name, "dep");
    assert_eq!(summary_id.version.to_string(), "0.5.0");
    assert_eq!(summary_id.source, SummarySource::crates_io());
    assert_eq!(
        *status,
        SummaryDiffStatus::Added {
            info: &PackageInfo {
                status: PackageStatus::Transitive,
                features: std_feature,
            },
        }
    );

    // Finally, foo.
    let (summary_id, status) = iter.next().expect("0 elements left");
    assert_eq!(summary_id.name, "foo");
    assert_eq!(summary_id.version.to_string(), "1.2.3");
    assert_eq!(summary_id.source, SummarySource::workspace("foo"));
    assert_eq!(
        *status,
        SummaryDiffStatus::Modified {
            old_version: None,
            old_source: None,
            old_status: None,
            new_status: PackageStatus::Initial,
            added_features: vec!["feature2"].into_iter().collect(),
            removed_features: BTreeSet::new(),
            unchanged_features: vec!["default", "feature1"].into_iter().collect(),
        }
    );

    // host_packages is:
    // * an insert + remove for bar, so it *should* be combined = 1 entry
    // * a change + insert for local-dep, so it should not be combined = 2 entries.
    assert_eq!(diff.host_packages.changed.len(), 3, "3 changed entries");
    let mut iter = diff.host_packages.changed.iter();

    // First, bar 0.2.0.
    let (summary_id, status) = iter.next().expect("2 elements left");
    assert_eq!(summary_id.name, "bar");
    assert_eq!(summary_id.version.to_string(), "0.2.0");
    assert_eq!(summary_id.source, SummarySource::workspace("dir/bar"));
    assert_eq!(
        *status,
        SummaryDiffStatus::Modified {
            old_version: Some(&Version::new(0, 1, 0)),
            old_source: None,
            old_status: Some(PackageStatus::Workspace),
            new_status: PackageStatus::Initial,
            added_features: BTreeSet::new(),
            removed_features: BTreeSet::new(),
            unchanged_features: vec!["default", "feature2"].into_iter().collect(),
        }
    );

    // Next, local-dep 1.1.2.
    let (summary_id, status) = iter.next().expect("2 elements left");
    assert_eq!(summary_id.name, "local-dep");
    assert_eq!(summary_id.version.to_string(), "1.1.2");
    assert_eq!(summary_id.source, SummarySource::path("../local-dep"));
    assert_eq!(
        *status,
        SummaryDiffStatus::Modified {
            old_version: None,
            old_source: None,
            old_status: None,
            new_status: PackageStatus::Transitive,
            added_features: vec!["dep-feature"].into_iter().collect(),
            removed_features: BTreeSet::new(),
            unchanged_features: BTreeSet::new(),
        }
    );

    // Finally, local-dep 2.0.0.
    let (summary_id, status) = iter.next().expect("1 element left");
    assert_eq!(summary_id.name, "local-dep");
    assert_eq!(summary_id.version.to_string(), "2.0.0");
    assert_eq!(summary_id.source, SummarySource::path("../local-dep-2"));
    assert_eq!(
        *status,
        SummaryDiffStatus::Added {
            info: &PackageInfo {
                status: PackageStatus::Transitive,
                features: BTreeSet::new(),
            },
        },
    );
}

#[test]
fn test_serialization() {
    let summary = Summary::parse(SERIALIZED_SUMMARY).expect("from_str succeeded");
    let summary2 = Summary::parse(SUMMARY2).expect("from_str succeeded");
    let diff = summary.diff(&summary2);

    let to_serialize = &diff;

    static EXPECTED_JSON: &str = r#"{"target-packages":{"changed":[{"name":"dep","version":"0.4.3","crates-io":true,"change":"added","status":"direct","features":["std"]},{"name":"dep","version":"0.5.0","crates-io":true,"change":"added","status":"transitive","features":["std"]},{"name":"foo","version":"1.2.3","workspace-path":"foo","change":"modified","old-version":null,"old-source":null,"old-status":null,"new-status":"initial","added-features":["feature2"],"removed-features":[],"unchanged-features":["default","feature1"]},{"name":"dep","version":"0.4.2","crates-io":true,"change":"removed","old-status":"direct","old-features":["std"]}],"unchanged":[{"name":"no-changes","version":"1.5.3","crates-io":true,"status":"transitive","features":["default"]}]},"host-packages":{"changed":[{"name":"local-dep","version":"2.0.0","path":"../local-dep-2","change":"added","status":"transitive","features":[]},{"name":"bar","version":"0.2.0","workspace-path":"dir/bar","change":"modified","old-version":"0.1.0","old-source":null,"old-status":"workspace","new-status":"initial","added-features":[],"removed-features":[],"unchanged-features":["default","feature2"]},{"name":"local-dep","version":"1.1.2","path":"../local-dep","change":"modified","old-version":null,"old-source":null,"old-status":null,"new-status":"transitive","added-features":["dep-feature"],"removed-features":[],"unchanged-features":[]}]}}"#;
    let j = serde_json::to_string(&to_serialize).expect("should serialize");
    println!("json output: {}", j);
    assert_eq!(j, EXPECTED_JSON);

    static EXPECTED_TOML: &str = r#"[[target-packages.changed]]
name = "dep"
version = "0.4.3"
crates-io = true
change = "added"
status = "direct"
features = ["std"]

[[target-packages.changed]]
name = "dep"
version = "0.5.0"
crates-io = true
change = "added"
status = "transitive"
features = ["std"]

[[target-packages.changed]]
name = "foo"
version = "1.2.3"
workspace-path = "foo"
change = "modified"
new-status = "initial"
added-features = ["feature2"]
removed-features = []
unchanged-features = ["default", "feature1"]

[[target-packages.changed]]
name = "dep"
version = "0.4.2"
crates-io = true
change = "removed"
old-status = "direct"
old-features = ["std"]

[[target-packages.unchanged]]
name = "no-changes"
version = "1.5.3"
crates-io = true
status = "transitive"
features = ["default"]
[[host-packages.changed]]
name = "local-dep"
version = "2.0.0"
path = "../local-dep-2"
change = "added"
status = "transitive"
features = []

[[host-packages.changed]]
name = "bar"
version = "0.2.0"
workspace-path = "dir/bar"
change = "modified"
old-version = "0.1.0"
old-status = "workspace"
new-status = "initial"
added-features = []
removed-features = []
unchanged-features = ["default", "feature2"]

[[host-packages.changed]]
name = "local-dep"
version = "1.1.2"
path = "../local-dep"
change = "modified"
new-status = "transitive"
added-features = ["dep-feature"]
removed-features = []
unchanged-features = []
"#;
    let toml_out = toml::to_string(&to_serialize).expect("should serialize");
    println!("toml output: {}", toml_out);
    assert_eq!(toml_out, EXPECTED_TOML);

    // TODO: add roundtrip test into the proper data structure. For now we just check that the output is valid TOML.
    let parsed = toml_out
        .parse::<toml::Value>()
        .expect("deserialization from value should work");
    println!("parsed output: {:?}", parsed);
}

fn make_summary(list: Vec<(SummaryId, PackageStatus, Vec<&str>)>) -> PackageMap {
    list.into_iter()
        .map(|(summary_id, status, features)| {
            let features = features
                .into_iter()
                .map(|feature| feature.to_string())
                .collect();
            (summary_id, PackageInfo { status, features })
        })
        .collect()
}
