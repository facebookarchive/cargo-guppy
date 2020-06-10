use assert_matches::assert_matches;
use cargo_metadata::{Metadata, Target};
use fixtures::json::JsonFixture;
use guppy::{graph::PackageGraph, Error};

#[test]
fn optional_dev_dep() {
    assert_invalid(
        include_str!("../../../fixtures/invalid/optional_dev_dep.json"),
        "dependency 'lazy_static' marked optional",
    );
}

#[test]
fn duplicate_workspace_names() {
    assert_invalid(
        include_str!("../../../fixtures/invalid/duplicate_workspace_names.json"),
        "duplicate package name in workspace: 'pkg' is name for",
    );
}

#[test]
fn build_targets_empty_kinds() {
    assert_invalid(
        include_str!("../../../fixtures/invalid/build_targets_empty_kinds.json"),
        "build target 'bench1' has no kinds",
    );
}

#[test]
fn build_targets_non_bin() {
    assert_invalid(
        include_str!("../../../fixtures/invalid/build_targets_non_bin.json"),
        "build target 'Binary(\"testcrate\")' has invalid crate types '{cdylib}'",
    );
}

#[test]
fn build_targets_duplicate_lib() {
    assert_invalid(
        include_str!("../../../fixtures/invalid/build_targets_duplicate_lib.json"),
        "duplicate build targets for Library",
    );
}

#[test]
fn proc_macro_mixed_kinds() {
    fn macro_target(metadata: &mut Metadata) -> &mut Target {
        let package = metadata
            .packages
            .iter_mut()
            .find(|p| p.name == "macro")
            .expect("valid package");
        package
            .targets
            .iter_mut()
            .find(|t| t.name == "macro")
            .expect("valid target")
    }

    let mut metadata: Metadata = serde_json::from_str(JsonFixture::metadata_proc_macro1().json())
        .expect("parsing metadata JSON should succeed");
    {
        let target = macro_target(&mut metadata);
        target.kind = vec!["lib".to_string(), "proc-macro".to_string()];
    }

    let json = serde_json::to_string(&metadata).expect("serializing worked");
    assert_invalid(&json, "proc-macro mixed with other kinds");

    {
        let target = macro_target(&mut metadata);

        // Reset target.kind to its old value.
        target.kind = vec!["proc-macro".to_string()];

        target.crate_types = vec!["lib".to_string(), "proc-macro".to_string()];
    }

    let json = serde_json::to_string(&metadata).expect("serializing worked");
    assert_invalid(&json, "proc-macro mixed with other crate types");
}

fn assert_invalid(json: &str, search_str: &str) {
    let err = PackageGraph::from_json(json).expect_err("expected error for invalid metadata");
    assert_matches!(
        err,
        Error::PackageGraphConstructError(ref s) if s.find(search_str).is_some(),
        "actual error is: {}", err,
    );
}
