use crate::{graph::PackageGraph, Error};
use assert_matches::assert_matches;

#[test]
fn optional_dev_dep() {
    assert_invalid(
        include_str!("../../fixtures/invalid/optional_dev_dep.json"),
        "dependency 'lazy_static' marked optional",
    );
}

fn assert_invalid(json: &str, search_str: &str) {
    let err = PackageGraph::from_json(json).expect_err("expected error for invalid metadata");
    assert_matches!(
        err,
        Error::PackageGraphConstructError(ref s) if s.find(search_str).is_some(),
        "actual error is: {}", err,
    );
}
