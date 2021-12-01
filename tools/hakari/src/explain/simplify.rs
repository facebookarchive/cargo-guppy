// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use itertools::Itertools;
#[cfg(feature = "cli-support")]
use owo_colors::{OwoColorize, Style};
#[cfg(feature = "cli-support")]
use std::fmt;
use std::{collections::BTreeSet, hash::Hash};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub(super) enum Simple<T> {
    Any,
    Some(T),
}

impl<T> Simple<T> {
    #[cfg(feature = "cli-support")]
    pub(super) fn display_with<'simple, F>(
        &'simple self,
        star_style: &'simple Style,
        display_fn: F,
    ) -> SimpleDisplay<'_, T, F>
    where
        F: Fn(&T, &mut fmt::Formatter) -> fmt::Result,
    {
        SimpleDisplay {
            simple: self,
            star_style,
            display_fn,
        }
    }
}

#[cfg(feature = "cli-support")]
pub(super) struct SimpleDisplay<'simple, T, F> {
    simple: &'simple Simple<T>,
    star_style: &'simple Style,
    display_fn: F,
}

#[cfg(feature = "cli-support")]
impl<'simple, T, F> fmt::Display for SimpleDisplay<'simple, T, F>
where
    F: Fn(&T, &mut fmt::Formatter) -> fmt::Result,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.simple {
            Simple::Any => write!(f, "{}", "*".style(*self.star_style)),
            Simple::Some(val) => (self.display_fn)(val, f),
        }
    }
}

pub(super) fn simplify3<A, B, C>(
    input: &BTreeSet<(A, B, C)>,
    (n_a, n_b, n_c): (usize, usize, usize),
) -> Vec<(Simple<A>, Simple<B>, Simple<C>)>
where
    A: Eq + Hash + Ord + Clone,
    B: Eq + Hash + Ord + Clone,
    C: Eq + Hash + Ord + Clone,
{
    // Do a super janky simplification right now
    // TODO: replace with a proper logic minimizer?

    if input.len() == (n_a * n_b * n_c) {
        return vec![(Simple::Any, Simple::Any, Simple::Any)];
    }

    let mut res = vec![];
    let group_map = input.iter().map(|(a, b, c)| (a, (b, c))).into_group_map();

    // It would be nice if into_group_map returned anything but HashMap:
    // https://github.com/rust-itertools/itertools/issues/520
    for (a, val) in group_map.into_iter().sorted() {
        if val.len() == n_b * n_c {
            res.push((Simple::Some(a.clone()), Simple::Any, Simple::Any));
        } else {
            for (b, val) in val.into_iter().into_group_map().into_iter().sorted() {
                if val.len() == n_c {
                    res.push((
                        Simple::Some(a.clone()),
                        Simple::Some(b.clone()),
                        Simple::Any,
                    ));
                } else {
                    for c in val {
                        res.push((
                            Simple::Some(a.clone()),
                            Simple::Some(b.clone()),
                            Simple::Some(c.clone()),
                        ));
                    }
                }
            }
        }
    }

    res
}

pub(super) fn simplify1<A>(input: &BTreeSet<A>, n_a: usize) -> Vec<Simple<A>>
where
    A: Eq + Hash + Ord + Clone,
{
    if input.len() == n_a {
        vec![Simple::Any]
    } else {
        input.iter().map(|a| Simple::Some(a.clone())).collect()
    }
}
