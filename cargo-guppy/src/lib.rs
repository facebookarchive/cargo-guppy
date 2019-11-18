use guppy::{diff, lockfile::Lockfile, Error};

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
