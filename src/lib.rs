pub mod diff;
mod errors;
pub mod lockfile;

pub use errors::Error;
pub use lockfile::{Lockfile, Package, PackageId};
use serde_json;

pub fn cmd_diff(json: bool, old: &str, new: &str) -> Result<(), Error> {
    let old = Lockfile::from_file(old)?;
    let new = Lockfile::from_file(new)?;

    let diff = diff::DiffOptions::default().diff(&old, &new);

    if json {
        println!("{}", serde_json::to_string_pretty(&diff).unwrap());
    } else {
        print!("{}", diff);
    }

    Ok(())
}

pub fn cmd_count() -> Result<(), Error> {
    let lockfile = Lockfile::from_file("Cargo.lock")?;

    println!("Third-party Packages: {}", lockfile.third_party_packages());

    Ok(())
}

pub fn cmd_dups() -> Result<(), Error> {
    let lockfile = Lockfile::from_file("Cargo.lock")?;

    lockfile.duplicate_packages();

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        diff::DiffOptions,
        lockfile::{Lockfile, PackageId},
    };

    #[test]
    fn it_works() {
        Lockfile::from_file("Cargo.lock").unwrap();
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

        let old: Lockfile = old.parse().unwrap();
        let new: Lockfile = new.parse().unwrap();

        let diff = DiffOptions::default().diff(&old, &new);

        serde_json::to_string(&diff).unwrap();
    }
}
