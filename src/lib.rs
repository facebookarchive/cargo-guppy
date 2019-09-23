// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(unused_variables)]
#![allow(dead_code)]
use serde::Deserialize;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::fs::File;
use std::io::Read;
use std::str::FromStr;
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
    type Err = ::std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let idx = s.find('+').ok_or(::std::io::ErrorKind::InvalidInput)?;
        let (source_type, url) = (&s[..idx], &s[idx + 1..]);

        match source_type {
            "registry" => Ok(Source::Registry(url.to_string())),
            "git" => {
                let idx = url.find('#').ok_or(::std::io::ErrorKind::InvalidInput)?;
                let (url, rev) = (&url[..idx], &url[idx + 1..]);

                Ok(Source::Git {
                    url: url.to_string(),
                    rev: rev.to_string(),
                })
            }
            _ => Err(::std::io::ErrorKind::InvalidInput.into()),
        }
    }
}

#[derive(Debug)]
struct Package {
    name: String,
    version: String,
    source: Source,
    checksum: Option<String>,
    dependencies: Option<Vec<PackageId>>,
}

impl Package {
    fn pkg_id(&self) -> PackageId {
        PackageId::new(
            self.name.clone(),
            self.version.clone(),
            self.source.get_url(),
        )
    }
}

#[derive(Debug)]
pub struct Lockfile {
    packages: HashMap<PackageId, Package>,
}

impl TryFrom<RawLockfile> for Lockfile {
    type Error = ::std::io::Error;

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

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
struct PackageId {
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
}

impl FromStr for PackageId {
    type Err = ::std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut iter = s.split_whitespace();
        let name = iter
            .next()
            .ok_or(::std::io::ErrorKind::InvalidInput)?
            .to_string();
        let version = iter
            .next()
            .ok_or(::std::io::ErrorKind::InvalidInput)?
            .to_string();
        let source = match iter.next() {
            Some(url) => {
                if url.starts_with('(') && url.ends_with(')') {
                    Some(url[1..url.len() - 1].to_string())
                } else {
                    return Err(::std::io::ErrorKind::InvalidInput.into());
                }
            }
            None => None,
        };

        Ok(Self::new(name, version, source))
    }
}

fn load_lockfile() {
    let path = "Cargo.lock";
    let mut file = File::open(path).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();

    //println!("{}", contents);

    let lockfile: toml::Value = toml::from_str(&contents).unwrap();
    //println!("{:#?}", lockfile);
    let lockfile: RawLockfile = toml::from_str(&contents).unwrap();
    //println!("{:#?}", lockfile);
    let lockfile: Lockfile = lockfile.try_into().unwrap();
    println!("{:#?}", lockfile);
}

pub fn diff_lockfiles(old: Lockfile, new: Lockfile) {
    let old = old.packages;
    let mut new = new.packages;

    let mut removed = Vec::new();
    for (pkg_id, _pkg) in old {
        if new.remove(&pkg_id).is_none() {
            removed.push(pkg_id);
        }
    }

    let added = new
        .into_iter()
        .map(|(pkg_id, _pkg)| pkg_id)
        .collect::<Vec<_>>();

    for pkg_id in removed {
        println!("Removed '{} {}'", pkg_id.name, pkg_id.version);
    }

    for pkg_id in added {
        println!("Added '{} {}'", pkg_id.name, pkg_id.version);
    }
}

pub fn cmd_diff(old: &str, new: &str) -> Result<(), ::std::io::Error> {
    let mut file = File::open(old).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();

    let old: Lockfile = toml::from_str::<RawLockfile>(&contents)
        .unwrap()
        .try_into()
        .unwrap();

    let mut file = File::open(new).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    let new: Lockfile = toml::from_str::<RawLockfile>(&contents)
        .unwrap()
        .try_into()
        .unwrap();

    diff_lockfiles(old, new);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        load_lockfile()
    }

    #[test]
    fn package_id_from_str() {
        let pkg = "serde 1.0.99 (registry+https://github.com/rust-lang/crates.io-index)"
            .parse()
            .unwrap();

        assert_eq!(
            PackageId::new(
                "serde".to_string(),
                "1.0.99".to_string(),
                Some("registry+https://github.com/rust-lang/crates.io-index".to_string())
            ),
            pkg
        );
    }

    #[test]
    fn simple_diff() {
        //
        let old = r#"
            [[package]]
            name = "cargo-guppy"
            version = "0.1.0"
            dependencies = [
             "serde 1.0.99 (registry+https://github.com/rust-lang/crates.io-index)",
             "toml 0.5.3 (registry+https://github.com/rust-lang/crates.io-index)",
            ]

            [[package]]
            name = "serde"
            version = "1.0.99"
            source = "registry+https://github.com/rust-lang/crates.io-index"

            [[package]]
            name = "toml"
            version = "0.5.3"
            source = "registry+https://github.com/rust-lang/crates.io-index"
            dependencies = [
             "serde 1.0.99 (registry+https://github.com/rust-lang/crates.io-index)",
            ]

            [metadata]
            "checksum serde 1.0.99 (registry+https://github.com/rust-lang/crates.io-index)" = "fec2851eb56d010dc9a21b89ca53ee75e6528bab60c11e89d38390904982da9f"
            "checksum toml 0.5.3 (registry+https://github.com/rust-lang/crates.io-index)" = "c7aabe75941d914b72bf3e5d3932ed92ce0664d49d8432305a8b547c37227724"
        "#;

        let new = r#"
            [[package]]
            name = "cargo-guppy"
            version = "0.1.0"
            dependencies = [
             "serde 1.0.99 (registry+https://github.com/rust-lang/crates.io-index)",
             "toml 0.5.3 (registry+https://github.com/rust-lang/crates.io-index)",
            ]

            [[package]]
            name = "proc-macro2"
            version = "1.0.2"
            source = "registry+https://github.com/rust-lang/crates.io-index"
            dependencies = [
             "unicode-xid 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)",
            ]

            [[package]]
            name = "quote"
            version = "1.0.2"
            source = "registry+https://github.com/rust-lang/crates.io-index"
            dependencies = [
             "proc-macro2 1.0.2 (registry+https://github.com/rust-lang/crates.io-index)",
            ]

            [[package]]
            name = "serde"
            version = "1.0.99"
            source = "registry+https://github.com/rust-lang/crates.io-index"
            dependencies = [
             "serde_derive 1.0.99 (registry+https://github.com/rust-lang/crates.io-index)",
            ]

            [[package]]
            name = "serde_derive"
            version = "1.0.99"
            source = "registry+https://github.com/rust-lang/crates.io-index"
            dependencies = [
             "proc-macro2 1.0.2 (registry+https://github.com/rust-lang/crates.io-index)",
             "quote 1.0.2 (registry+https://github.com/rust-lang/crates.io-index)",
             "syn 1.0.5 (registry+https://github.com/rust-lang/crates.io-index)",
            ]

            [[package]]
            name = "syn"
            version = "1.0.5"
            source = "registry+https://github.com/rust-lang/crates.io-index"
            dependencies = [
             "proc-macro2 1.0.2 (registry+https://github.com/rust-lang/crates.io-index)",
             "quote 1.0.2 (registry+https://github.com/rust-lang/crates.io-index)",
             "unicode-xid 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)",
            ]

            [[package]]
            name = "unicode-xid"
            version = "0.2.0"
            source = "registry+https://github.com/rust-lang/crates.io-index"

            [metadata]
            "checksum proc-macro2 1.0.2 (registry+https://github.com/rust-lang/crates.io-index)" = "175a40b9cf564ce9bf050654633dbf339978706b8ead1a907bb970b63185dd95"
            "checksum quote 1.0.2 (registry+https://github.com/rust-lang/crates.io-index)" = "053a8c8bcc71fcce321828dc897a98ab9760bef03a4fc36693c231e5b3216cfe"
            "checksum serde 1.0.99 (registry+https://github.com/rust-lang/crates.io-index)" = "fec2851eb56d010dc9a21b89ca53ee75e6528bab60c11e89d38390904982da9f"
            "checksum serde_derive 1.0.99 (registry+https://github.com/rust-lang/crates.io-index)" = "cb4dc18c61206b08dc98216c98faa0232f4337e1e1b8574551d5bad29ea1b425"
            "checksum syn 1.0.5 (registry+https://github.com/rust-lang/crates.io-index)" = "66850e97125af79138385e9b88339cbcd037e3f28ceab8c5ad98e64f0f1f80bf"
            "checksum unicode-xid 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)" = "826e7639553986605ec5979c7dd957c7895e93eabed50ab2ffa7f6128a75097c"
        "#;

        let old: Lockfile = toml::from_str::<RawLockfile>(&old)
            .unwrap()
            .try_into()
            .unwrap();
        let new: Lockfile = toml::from_str::<RawLockfile>(&new)
            .unwrap()
            .try_into()
            .unwrap();

        diff_lockfiles(old, new);
    }
}
