use crate::Error;
use serde::{
    de::{Error as _, IntoDeserializer},
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{collections::HashMap, fs::File, io::Read, str::FromStr};
use toml;

#[derive(Debug, Deserialize, Serialize)]
pub struct Lockfile {
    package: Vec<Package>,
    metadata: HashMap<String, String>,
}

impl Lockfile {
    pub fn from_file(path: &str) -> Result<Self, Error> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        Self::from_str(&contents)
    }

    pub fn from_str(s: &str) -> Result<Self, Error> {
        Ok(toml::from_str(s).map_err(Error::LockfileParseError)?)
    }

    pub fn packages(&self) -> impl Iterator<Item = &Package> {
        self.package.iter()
    }

    pub fn third_party_packages(&self) -> usize {
        self.packages()
            .filter(|pkg| {
                if let Source::Path = pkg.source {
                    false
                } else {
                    true
                }
            })
            .count()
    }

    pub fn duplicate_packages(&self) {
        let mut map = HashMap::new();

        for pkg in self.packages() {
            map.entry(pkg.name())
                .or_insert(Vec::new())
                .push(pkg.package_id());
        }

        for (name, duplicates) in map {
            if duplicates.len() <= 1 {
                continue;
            }

            print!("{} ({}", name, duplicates[0].version());

            for pkg_id in &duplicates[1..] {
                print!(", {}", pkg_id.version());
            }
            println!(")");
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Package {
    name: String,
    version: String,
    #[serde(default, skip_serializing_if = "Source::is_path")]
    source: Source,
    dependencies: Option<Vec<PackageId>>,
}

impl Package {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn package_id(&self) -> PackageId {
        PackageId::new(
            self.name.clone(),
            self.version.clone(),
            self.source.get_url(),
        )
    }
}

#[derive(Clone, Debug)]
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

    fn is_path(&self) -> bool {
        match self {
            Source::Path => true,
            _ => false,
        }
    }
}

impl Default for Source {
    fn default() -> Self {
        Source::Path
    }
}

impl Serialize for Source {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Source::Path => s.serialize_none(),
            Source::Registry(url) => s.serialize_str(&format!("registry+{}", url)),
            Source::Git { url, rev } => s.serialize_str(&format!("git+{}#{}", url, rev)),
        }
    }
}

impl<'de> Deserialize<'de> for Source {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        let idx = s.find('+').ok_or(D::Error::custom("Invalid Input"))?;
        let (source_type, url) = (&s[..idx], &s[idx + 1..]);

        match source_type {
            "registry" => Ok(Source::Registry(url.to_string())),
            "git" => {
                let idx = url.find('#').ok_or(D::Error::custom("Invalid Input"))?;
                let (url, rev) = (&url[..idx], &url[idx + 1..]);

                Ok(Source::Git {
                    url: url.to_string(),
                    rev: rev.to_string(),
                })
            }
            _ => Err(D::Error::custom("Invalid Input")),
        }
    }
}

impl FromStr for Source {
    type Err = serde::de::value::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::deserialize(s.into_deserializer())
    }
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

impl Serialize for PackageId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = format!("{} {}", self.name, self.version);

        if let Some(source) = &self.source {
            s.push_str(&format!(" ({})", source));
        }
        serializer.serialize_str(&s)
    }
}

impl<'de> Deserialize<'de> for PackageId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        let mut iter = s.split_whitespace();
        let name = iter
            .next()
            .ok_or(D::Error::custom("Invalid Input"))?
            .to_string();
        let version = iter
            .next()
            .ok_or(D::Error::custom("Invalid Input"))?
            .to_string();
        let source = match iter.next() {
            Some(url) => {
                if url.starts_with('(') && url.ends_with(')') {
                    Some(url[1..url.len() - 1].to_string())
                } else {
                    return Err(D::Error::custom("Invalid Input"));
                }
            }
            None => None,
        };

        Ok(Self::new(name, version, source))
    }
}

impl FromStr for PackageId {
    type Err = serde::de::value::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::deserialize(s.into_deserializer())
    }
}

#[cfg(test)]
mod tests {
    use crate::lockfile::{Lockfile, PackageId};

    #[test]
    fn from_file() {
        Lockfile::from_file("../Cargo.lock").unwrap();
    }

    #[test]
    fn package_id_from_str() {
        let s = "serde 1.0.99 (registry+https://github.com/rust-lang/crates.io-index)";
        let pkg: PackageId = s.parse().unwrap();

        assert_eq!(pkg.name(), "serde");
        assert_eq!(pkg.version(), "1.0.99");
        assert_eq!(
            pkg.source(),
            Some("registry+https://github.com/rust-lang/crates.io-index")
        );
    }
}
