# guppy-summaries

[![guppy-summaries on crates.io](https://img.shields.io/crates/v/guppy-summaries)](https://crates.io/crates/guppy-summaries) [![Documentation (latest release)](https://docs.rs/guppy-summaries/badge.svg)](https://docs.rs/guppy-summaries/) [![Documentation (main)](https://img.shields.io/badge/docs-main-brightgreen)](https://facebookincubator.github.io/cargo-guppy/rustdoc/guppy_summaries/) [![License](https://img.shields.io/badge/license-Apache-green.svg)](../LICENSE-APACHE) [![License](https://img.shields.io/badge/license-MIT-green.svg)](../LICENSE-MIT)

Facilities to serialize, deserialize and compare build summaries.

A *build summary* is a record of what packages and features are built on the target and host
platforms. A summary file can be checked into a repository, kept up to date and compared in CI,
and allow for tracking results of builds over time.

`guppy-summaries` is designed to be small and independent of the main `guppy` crate.

## Examples

```rust
use guppy_summaries::{Summary, SummaryId, SummarySource, PackageStatus};
use pretty_assertions::assert_eq;
use semver::Version;
use std::collections::BTreeSet;
use toml::Value;

// A summary is a TOML file that has this format:
static SUMMARY: &str = r#"
[[target-package]]
name = "foo"
version = "1.2.3"
workspace-path = "foo"
status = 'initial'
features = ["feature-a", "feature-c"]

[[host-package]]
name = "proc-macro"
version = "0.1.2"
workspace-path = "proc-macros/macro"
status = 'workspace'
features = ["macro-expand"]

[[host-package]]
name = "bar"
version = "0.4.5"
crates-io = true
status = 'direct'
features = []
"#;

// The summary can be deserialized:
let summary = Summary::parse(SUMMARY).expect("from_str succeeded");

// ... and a package and its features can be looked up.
let summary_id = SummaryId::new("foo", Version::new(1, 2, 3), SummarySource::workspace("foo"));
let info = &summary.target_packages[&summary_id];
assert_eq!(info.status, PackageStatus::Initial, "correct status");
assert_eq!(
    info.features.iter().map(|feature| feature.as_str()).collect::<Vec<_>>(),
    ["feature-a", "feature-c"],
    "correct feature list"
);

// Another summary.
static SUMMARY2: &str = r#"
[[target-package]]
name = "foo"
version = "1.2.4"
workspace-path = "new-location/foo"
status = 'initial'
features = ["feature-a", "feature-b"]

[[target-package]]
name = "once_cell"
version = "1.4.0"
source = "git+https://github.com/matklad/once_cell?tag=v1.4.0"
status = 'transitive'
features = ["std"]

[[host-package]]
name = "bar"
version = "0.4.5"
crates-io = true
status = 'direct'
features = []
"#;

let summary2 = Summary::parse(SUMMARY2).expect("from_str succeeded");

// Diff summary and summary2.
let diff = summary.diff(&summary2);

// Pretty-print a report generated from the diff.
let diff_str = format!("{}", diff.report());
assert_eq!(
    r#"target packages:
  A once_cell 1.4.0 (transitive third-party, external 'git+https://github.com/matklad/once_cell?tag=v1.4.0')
    * features: std
  M foo 1.2.4 (initial, path 'new-location/foo')
    * version upgraded from 1.2.3
    * source changed from path 'foo'
    * added features: feature-b
    * removed features: feature-c
    * (unchanged features: feature-a)

host packages:
  R proc-macro 0.1.2 (workspace, path 'proc-macros/macro')
    * (old features: macro-expand)

"#,
    diff_str,
);
```

## Contributing

See the [CONTRIBUTING](../CONTRIBUTING.md) file for how to help out.

## License

This project is available under the terms of either the [Apache 2.0 license](../LICENSE-APACHE) or the [MIT
license](../LICENSE-MIT).

<!--
README.md is generated from README.tpl by cargo readme. To regenerate:

cargo install cargo-readme
cargo readme > README.md
-->
