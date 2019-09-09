// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::Error;
use serde::Deserialize;
use std::{
    collections::HashMap,
    convert::{TryFrom, TryInto},
    fs::File,
    io::Read,
    str::FromStr,
};
use toml;

#[derive(Debug, Deserialize)]
struct RawLockfile {
    metadata: HashMap<String, String>,
    package: Vec<RawPackage>,
}

#[derive(Debug, Deserialize)]
struct RawPackage {
    name: String,
    version: String,
    source: Option<String>,
    dependencies: Option<Vec<String>>,
}

#[derive(Debug)]
enum Source {
    Path,
    Registry(String),
    Git { url: String, rev: String },
}

impl Source {
    fn get_url(&self) -> Option<String> {
        match &self {
            Source::Path => None,
            Source::Registry(url) => Some(format!("registry+{}", url)),
            Source::Git { url, .. } => Some(format!("git+{}", url)),
        }
    }
}

impl FromStr for Source {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let idx = s.find('+').ok_or(Error::InvalidInput)?;
        let (source_type, url) = (&s[..idx], &s[idx + 1..]);

        match source_type {
            "registry" => Ok(Source::Registry(url.to_string())),
            "git" => {
                let idx = url.find('#').ok_or(Error::InvalidInput)?;
                let (url, rev) = (&url[..idx], &url[idx + 1..]);

                Ok(Source::Git {
                    url: url.to_string(),
                    rev: rev.to_string(),
                })
            }
            _ => Err(Error::InvalidInput),
        }
    }
}

#[derive(Debug)]
pub struct Package {
    name: String,
    version: String,
    source: Source,
    checksum: Option<String>,
    dependencies: Option<Vec<PackageId>>,
}

impl Package {
    pub fn pkg_id(&self) -> PackageId {
        PackageId::new(
            self.name.clone(),
            self.version.clone(),
            self.source.get_url(),
        )
    }
}

#[derive(Debug)]
pub struct Lockfile {
    pub packages: HashMap<PackageId, Package>,
}

impl TryFrom<RawLockfile> for Lockfile {
    type Error = Error;

    fn try_from(value: RawLockfile) -> Result<Self, Self::Error> {
        let mut checksums = value
            .metadata
            .into_iter()
            .filter(|(k, _)| k.starts_with("checksum "))
            .map(|(k, v)| {
                let k = k.trim_start_matches("checksum ");
                let pkg_id: PackageId = k.parse()?;
                Ok((pkg_id, v))
            })
            .collect::<Result<HashMap<_, _>, Self::Error>>()?;

        let packages = value
            .package
            .into_iter()
            .map(|raw_pkg| {
                let source = match raw_pkg.source {
                    None => Source::Path,
                    Some(s) => s.parse()?,
                };
                let pkg_id = PackageId::new(
                    raw_pkg.name.clone(),
                    raw_pkg.version.clone(),
                    source.get_url(),
                );
                let checksum = checksums.remove(&pkg_id);
                let dependencies = match raw_pkg.dependencies {
                    None => None,
                    Some(deps) => Some(deps.into_iter().map(|s| s.parse()).collect::<Result<
                        Vec<PackageId>,
                        Self::Error,
                    >>(
                    )?),
                };

                let package = Package {
                    name: raw_pkg.name,
                    version: raw_pkg.version,
                    source,
                    checksum,
                    dependencies,
                };

                Ok((pkg_id, package))
            })
            .collect::<Result<HashMap<_, _>, Self::Error>>()?;

        // Maybe check that checksums is empty?

        Ok(Lockfile { packages })
    }
}

impl FromStr for Lockfile {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        toml::from_str::<RawLockfile>(s)
            .map_err(|_| Error::InvalidInput)?
            .try_into()
    }
}

pub fn load_lockfile(path: &str) -> Result<Lockfile, Error> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    contents.parse()
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub struct PackageId {
    name: String,
    version: String,
    source: Option<String>,
}

impl PackageId {
    pub fn new(name: String, version: String, source: Option<String>) -> Self {
        Self {
            name,
            version,
            source,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn source(&self) -> Option<&str> {
        self.source.as_ref().map(|s| s.as_ref())
    }
}

impl FromStr for PackageId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut iter = s.split_whitespace();
        let name = iter.next().ok_or(Error::InvalidInput)?.to_string();
        let version = iter.next().ok_or(Error::InvalidInput)?.to_string();
        let source = match iter.next() {
            Some(url) => {
                if url.starts_with('(') && url.ends_with(')') {
                    Some(url[1..url.len() - 1].to_string())
                } else {
                    return Err(Error::InvalidInput);
                }
            }
            None => None,
        };

        Ok(Self::new(name, version, source))
    }
}
