// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::parser::parse;
use crate::types::{Atom, Expr, Target};

#[test]
fn test_triple() {
    let res = parse("x86_64-apple-darwin");
    assert_eq!(res, Ok(Target::Triple("x86_64-apple-darwin".to_string())));
}

#[test]
fn test_single() {
    assert_eq!(
        parse("cfg(windows)"),
        Ok(Target::Spec(Expr::TestSet(Atom::Ident(
            "windows".to_string()
        )))),
    );
}

#[test]
fn test_not() {
    assert_eq!(
        parse("cfg(not(windows))"),
        Ok(Target::Spec(Expr::Not(Box::new(Expr::TestSet(
            Atom::Ident("windows".to_string())
        ))))),
    );
}

#[test]
fn test_testequal() {
    assert_eq!(
        parse("cfg(target_os = \"windows\")"),
        Ok(Target::Spec(Expr::TestEqual((
            Atom::Ident("target_os".to_string()),
            Atom::Value("windows".to_string())
        )))),
    );
}
