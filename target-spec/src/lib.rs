// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

mod evaluator;
mod parser;
#[cfg(test)]
mod tests;
mod types;

pub use evaluator::eval;
