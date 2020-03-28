// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::parser::parse;
use crate::types::TargetEnum;
use cfg_expr::targets::{Family, Os};
use cfg_expr::{Predicate, TargetPredicate};

#[test]
fn test_triple() {
    let res = parse("x86_64-apple-darwin");
    assert!(matches!(
        res,
        Ok(TargetEnum::TargetInfo(target_info)) if target_info.triple == "x86_64-apple-darwin"
    ));
}

#[test]
fn test_single() {
    let expr = match parse("cfg(windows)").unwrap() {
        TargetEnum::TargetInfo(target_info) => {
            panic!("expected spec, got target info: {:?}", target_info)
        }
        TargetEnum::Spec(expr) => expr,
    };
    assert_eq!(
        expr.predicates().collect::<Vec<_>>(),
        vec![Predicate::Target(TargetPredicate::Family(Some(
            Family::windows
        )))],
    );
}

#[test]
fn test_not() {
    assert!(matches!(
        parse("cfg(not(windows))"),
        Ok(TargetEnum::Spec(_))
    ));
}

#[test]
fn test_testequal() {
    let expr = match parse("cfg(target_os = \"windows\")").unwrap() {
        TargetEnum::TargetInfo(target_info) => {
            panic!("expected spec, got target info: {:?}", target_info)
        }
        TargetEnum::Spec(expr) => expr,
    };

    assert_eq!(
        expr.predicates().collect::<Vec<_>>(),
        vec![Predicate::Target(TargetPredicate::Os(Some(Os::windows)))],
    );
}

#[test]
fn test_extra() {
    let res = parse("cfg(unix)this-is-extra");
    res.expect_err("extra content at the end");
}

#[test]
fn test_incomplete() {
    // This fails because the ) at the end is missing.
    let res = parse("cfg(not(unix)");
    res.expect_err("missing ) at the end");
}
