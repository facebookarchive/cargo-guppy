// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::errors::Error;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Config {
    /// Groups defined.
    pub groups: HashMap<String, Vec<String>>,
}

//#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
//pub struct Group {
//    pub members: Vec<String>,
//}

impl Config {
    /// Parses and constructs a config from a file.
    pub fn from_file(f: impl AsRef<Path>) -> Result<Self, Error> {
        let contents = fs::read(f).map_err(Error::ConfigIoError)?;
        Self::from_toml(&contents)
    }

    pub fn from_toml(bytes: &[u8]) -> Result<Self, Error> {
        toml::from_slice(bytes).map_err(Error::ConfigParseError)
    }
}
