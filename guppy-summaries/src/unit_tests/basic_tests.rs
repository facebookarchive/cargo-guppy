// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{PackageMap, Summary, SummaryDiffStatus, SummaryId, SummarySource};
use pretty_assertions::assert_eq;
use semver::Version;
use std::collections::BTreeSet;

#[test]
fn basic_roundtrip() {
    let target_initials = vec![(
        SummaryId::new(
            "foo",
            Version::new(1, 2, 3),
            SummarySource::workspace("foo"),
        ),
        vec!["default", "feature1"],
    )];
    let host_initials = vec![(
        SummaryId::new(
            "bar",
            Version::new(0, 1, 0),
            SummarySource::workspace("dir/bar"),
        ),
        vec!["default", "feature2"],
    )];
    let target_packages = vec![(
        SummaryId::new("dep", Version::new(0, 4, 2), SummarySource::crates_io()),
        vec!["std"],
    )];
    let host_packages = vec![(
        SummaryId::new(
            "local-dep",
            Version::new(1, 1, 2),
            SummarySource::path("../local-dep"),
        ),
        vec![],
    )];

    let summary = Summary {
        metadata: None,
        target_initials: make_summary(target_initials),
        host_initials: make_summary(host_initials),
        target_packages: make_summary(target_packages),
        host_packages: make_summary(host_packages),
    };

    let mut s = "# This is a test @generated summary.\n\n".to_string();
    summary.write_to_string(&mut s).expect("write succeeded");

    static SERIALIZED_SUMMARY: &str = r#"# This is a test @generated summary.

[[target-initial]]
name = 'foo'
version = '1.2.3'
workspace-path = 'foo'
features = ['default', 'feature1']

[[host-initial]]
name = 'bar'
version = '0.1.0'
workspace-path = 'dir/bar'
features = ['default', 'feature2']

[[target-package]]
name = 'dep'
version = '0.4.2'
crates-io = true
features = ['std']

[[host-package]]
name = 'local-dep'
version = '1.1.2'
path = '../local-dep'
features = []
"#;
    assert_eq!(&s, SERIALIZED_SUMMARY, "serialized representation matches");

    let deserialized = Summary::parse(&s).expect("from_str succeeded");
    assert_eq!(summary, deserialized, "deserialized representation matches");

    let diff = summary.diff(&deserialized);
    assert!(diff.is_unchanged(), "diff should be empty");

    // Try changing some things.
    static SUMMARY2: &str = r#"# This is a test @generated summary.

[[target-initial]]
name = 'foo'
version = '1.2.3'
workspace-path = 'foo'
features = ['default', 'feature1', 'feature2']

[[host-initial]]
name = 'bar'
version = '0.2.0'
workspace-path = 'dir/bar'
features = ['default', 'feature2']

[[target-package]]
name = 'dep'
version = '0.4.3'
crates-io = true
features = ['std']

[[target-package]]
name = 'dep'
version = '0.5.0'
crates-io = true
features = ['std']

[[host-package]]
name = 'local-dep'
version = '1.1.2'
path = '../local-dep'
features = ['dep-feature']

[[host-package]]
name = 'local-dep'
version = '2.0.0'
path = '../local-dep-2'
features = []
"#;

    let summary2 = Summary::parse(SUMMARY2).expect("from_str succeeded");
    let diff = summary.diff(&summary2);

    assert_eq!(diff.target_initials.changed.len(), 1, "1 changed entry");
    let (summary_id, status) = diff
        .target_initials
        .changed
        .iter()
        .next()
        .expect("target_initials has 1 element");
    assert_eq!(summary_id.name, "foo");
    assert_eq!(summary_id.version.to_string(), "1.2.3");
    assert_eq!(summary_id.source, SummarySource::workspace("foo"));
    assert_eq!(
        *status,
        SummaryDiffStatus::Changed {
            old_version: None,
            old_source: None,
            added_features: vec!["feature2"].into_iter().collect(),
            removed_features: BTreeSet::new(),
            unchanged_features: vec!["default", "feature1"].into_iter().collect(),
        }
    );

    // bar is an insert and a remove, so it should be combined.
    assert_eq!(diff.host_initials.changed.len(), 1, "1 changed entry");
    let (summary_id, status) = diff
        .host_initials
        .changed
        .iter()
        .next()
        .expect("host_initials has 1 element");
    assert_eq!(summary_id.name, "bar");
    assert_eq!(summary_id.version.to_string(), "0.2.0");
    assert_eq!(summary_id.source, SummarySource::workspace("dir/bar"));
    assert_eq!(
        *status,
        SummaryDiffStatus::Changed {
            old_version: Some(&Version::new(0, 1, 0)),
            old_source: None,
            added_features: BTreeSet::new(),
            removed_features: BTreeSet::new(),
            unchanged_features: vec!["default", "feature2"].into_iter().collect(),
        }
    );

    // target_packages is a remove + 2 inserts for dep, so it should not be combined.
    assert_eq!(diff.target_packages.changed.len(), 3, "3 changed entries");
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
            old_features: &std_feature
        }
    );

    // Next, dep 0.4.3.
    let (summary_id, status) = iter.next().expect("2 elements left");
    assert_eq!(summary_id.name, "dep");
    assert_eq!(summary_id.version.to_string(), "0.4.3");
    assert_eq!(summary_id.source, SummarySource::crates_io());
    assert_eq!(
        *status,
        SummaryDiffStatus::Added {
            features: &std_feature
        }
    );

    // Finally, dep 0.5.0.
    let (summary_id, status) = iter.next().expect("1 element left");
    assert_eq!(summary_id.name, "dep");
    assert_eq!(summary_id.version.to_string(), "0.5.0");
    assert_eq!(summary_id.source, SummarySource::crates_io());
    assert_eq!(
        *status,
        SummaryDiffStatus::Added {
            features: &std_feature
        }
    );

    // host_packages is a change + insert, so it should not be combined.
    assert_eq!(diff.host_packages.changed.len(), 2, "2 changed entries");
    let mut iter = diff.host_packages.changed.iter();

    // First, local-dep 1.1.2.
    let (summary_id, status) = iter.next().expect("2 elements left");
    assert_eq!(summary_id.name, "local-dep");
    assert_eq!(summary_id.version.to_string(), "1.1.2");
    assert_eq!(summary_id.source, SummarySource::path("../local-dep"));
    assert_eq!(
        *status,
        SummaryDiffStatus::Changed {
            old_version: None,
            old_source: None,
            added_features: vec!["dep-feature"].into_iter().collect(),
            removed_features: BTreeSet::new(),
            unchanged_features: BTreeSet::new(),
        }
    );

    // Next, local-dep 2.0.0.
    let (summary_id, status) = iter.next().expect("1 element left");
    assert_eq!(summary_id.name, "local-dep");
    assert_eq!(summary_id.version.to_string(), "2.0.0");
    assert_eq!(summary_id.source, SummarySource::path("../local-dep-2"));
    assert_eq!(
        *status,
        SummaryDiffStatus::Added {
            features: &BTreeSet::new(),
        }
    );
}

fn make_summary(list: Vec<(SummaryId, Vec<&str>)>) -> PackageMap {
    list.into_iter()
        .map(|(summary_id, features)| {
            (
                summary_id,
                features
                    .into_iter()
                    .map(|feature| feature.to_string())
                    .collect(),
            )
        })
        .collect()
}
