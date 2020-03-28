// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::parser::parse_impl;
use crate::types::{Atom, Expr, TargetEnum};
use nom::error::ErrorKind;

#[test]
fn test_triple() {
    let res = parse_impl("x86_64-apple-darwin");
    assert_eq!(
        res,
        Ok(TargetEnum::Triple("x86_64-apple-darwin".to_string()))
    );
}

#[test]
fn test_single() {
    assert_eq!(
        parse_impl("cfg(windows)"),
        Ok(TargetEnum::Spec(Expr::TestSet(Atom::Ident(
            "windows".to_string()
        )))),
    );
}

#[test]
fn test_not() {
    assert_eq!(
        parse_impl("cfg(not(windows))"),
        Ok(TargetEnum::Spec(Expr::Not(Box::new(Expr::TestSet(
            Atom::Ident("windows".to_string())
        ))))),
    );
}

#[test]
fn test_testequal() {
    assert_eq!(
        parse_impl("cfg(target_os = \"windows\")"),
        Ok(TargetEnum::Spec(Expr::TestEqual((
            Atom::Ident("target_os".to_string()),
            Atom::Value("windows".to_string())
        )))),
    );
}

#[test]
fn test_extra() {
    let res = parse_impl("cfg(unix)this-is-extra");
    assert_eq!(res, Err(nom::Err::Error(("this-is-extra", ErrorKind::Eof))));
}

#[test]
fn test_incomplete() {
    // This fails because the ) at the end is missing.
    let res = parse_impl("cfg(not(unix)");
    assert_eq!(res, Err(nom::Err::Failure(("", ErrorKind::Char))));
}
