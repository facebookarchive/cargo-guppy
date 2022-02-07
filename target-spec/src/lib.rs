// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Evaluate `Cargo.toml` target specifications against platform triples.
//!
//! Cargo supports
//! [platform-specific dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#platform-specific-dependencies).
//! These dependencies can be specified in one of two ways:
//!
//! ```toml
//! # 1. As Rust-like `#[cfg]` syntax.
//! [target.'cfg(all(unix, target_arch = "x86_64"))'.dependencies]
//! native = { path = "native/x86_64" }
//!
//! # 2. Listing out the full target triple.
//! [target.x86_64-pc-windows-gnu.dependencies]
//! winhttp = "0.4.0"
//! ```
//!
//! `target-spec` provides the `eval` API which can be used to figure out whether such a
//! dependency will be included on a particular platform.
//!
//! ```rust
//! use target_spec::eval;
//!
//! // Evaluate Rust-like `#[cfg]` syntax.
//! let cfg_target = "cfg(all(unix, target_arch = \"x86_64\"))";
//! assert_eq!(eval(cfg_target, "x86_64-unknown-linux-gnu"), Ok(Some(true)));
//! assert_eq!(eval(cfg_target, "i686-unknown-linux-gnu"), Ok(Some(false)));
//! assert_eq!(eval(cfg_target, "x86_64-pc-windows-msvc"), Ok(Some(false)));
//!
//! // Evaluate a full target-triple.
//! assert_eq!(eval("x86_64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"), Ok(Some(true)));
//! assert_eq!(eval("x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc"), Ok(Some(false)));
//! ```
//!
//! For more advanced usage, see [`Platform`] and [`TargetSpec`].
//!
//! ## Optional features
//!
//! * **`summaries`**: Adds the [`summaries`] module to enable serialization of [`Platform`] and [`TargetFeatures`].
//! * **`proptest1`**: Enables support for property-based testing of [`Platform`] and
//!   [`TargetFeatures`] using [`proptest`].
//!
//! ## Minimum supported Rust version
//!
//! The minimum supported Rust version (MSRV) is **Rust 1.54**. Within the 1.x series, MSRV bumps will
//! be accompanied by a minor version update.

#![warn(missing_docs)]
#![forbid(unsafe_code)]
#![cfg_attr(doc_cfg, feature(doc_cfg, doc_auto_cfg))]

pub mod errors;
mod platform;
#[cfg(feature = "proptest1")]
mod proptest_helpers;
mod simple_eval;
mod spec;
#[cfg(feature = "summaries")]
pub mod summaries;
mod triple;

pub use errors::Error;
pub use platform::*;
pub use simple_eval::*;
pub use spec::*;
pub use triple::*;
