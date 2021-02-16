// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{anyhow, bail, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use dialoguer::Confirm;
use guppy::graph::{PackageGraph, PackageLink, PackageMetadata};
use guppy_cmdlib::CargoMetadataOptions;
use pathdiff::diff_paths;
use std::{
    collections::{btree_map::Entry, BTreeMap, HashSet},
    fmt, fs,
    io::{self, Write},
    mem,
    path::{Path, PathBuf, MAIN_SEPARATOR},
};
use structopt::StructOpt;
use toml_edit::{decorated, Document, Item, Table, Value};

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab")]
pub struct MvOptions {
    /// Source directories to move
    #[structopt(name = "DIR", required = true)]
    src_dirs: Vec<Utf8PathBuf>,

    /// Destination directory to move to
    #[structopt(name = "DEST")]
    dest_dir: Utf8PathBuf,

    /// Print out operations instead of performing them
    #[structopt(long)]
    dry_run: bool,

    #[structopt(flatten)]
    metadata_opts: CargoMetadataOptions,
}

impl MvOptions {
    pub fn exec(&self) -> Result<()> {
        // Construct a package graph.
        let command = self.metadata_opts.make_command();
        let pkg_graph = command.build_graph()?;
        let workspace_root = pkg_graph.workspace().root();

        let dest_dir = DestDir::new(&pkg_graph, &self.dest_dir)?;

        if dest_dir.is_create() && self.src_dirs.len() > 1 {
            bail!("multiple sources specified with a destination that doesn't exist");
        }

        // Each source directory maps to one or more packages.
        let mut src_moves = BTreeMap::new();
        for src_dir in &self.src_dirs {
            let src_dir = canonicalize_dir(&pkg_graph, src_dir)?;
            for (workspace_path, package_move) in moves_for(&pkg_graph, &src_dir, &dest_dir)? {
                match src_moves.entry(workspace_path) {
                    // This disallows, e.g. "cargo guppy mv foo foo/bar dest"
                    Entry::Occupied(_) => bail!(
                        "workspace path '{}' specified multiple times in source",
                        workspace_path
                    ),
                    Entry::Vacant(v) => {
                        v.insert(package_move);
                    }
                }
            }
        }

        // Build a map of edits to perform (manifest path to a list of edits).
        let mut manifest_edits: BTreeMap<&Utf8Path, Vec<_>> = BTreeMap::new();

        for package_move in src_moves.values() {
            for link in package_move.package.direct_links() {
                let (from, to) = link.endpoints();
                let old_path = if let Some(path) = to.source().workspace_path() {
                    path
                } else {
                    continue;
                };

                // If the 'to' moves as well, let the below loop deal with it.
                if src_moves.contains_key(old_path) {
                    continue;
                }

                let edit_path = diff_paths(old_path, &package_move.new_path)
                    .expect("paths are all relative so diff_paths can never return None");

                let edit_path = check_utf8_path(edit_path)?;
                manifest_edits
                    .entry(from.manifest_path())
                    .or_default()
                    .push(ManifestEdit { link, edit_path });
            }

            for link in package_move.package.reverse_direct_links() {
                let from = link.from();
                let old_path = from
                    .source()
                    .workspace_path()
                    .expect("reverse deps of workspace packages must be in workspace");
                // If the 'from' moves as well, compute the new path based on that.
                let edit_path = if let Some(from_move) = src_moves.get(old_path) {
                    diff_paths(&package_move.new_path, &from_move.new_path)
                } else {
                    diff_paths(&package_move.new_path, old_path)
                }
                .expect("paths are all relative so diff_paths can never return None");

                let edit_path = check_utf8_path(edit_path)?;
                manifest_edits
                    .entry(from.manifest_path())
                    .or_default()
                    .push(ManifestEdit { link, edit_path });
            }
        }

        println!("Will perform edits:");
        for (manifest_path, edits) in &manifest_edits {
            println!(
                "manifest: {}",
                diff_paths(manifest_path, workspace_root).unwrap().display()
            );
            for edit in edits {
                println!("  * {}", edit);
            }
        }

        println!("\nMoves:");
        for (src_dir, package_move) in &src_moves {
            println!("  * move {} to {}", src_dir, package_move.new_path,);
        }

        println!();

        if self.dry_run {
            return Ok(());
        }

        let perform = Confirm::new()
            .with_prompt("Continue?")
            .show_default(true)
            .interact()?;

        if perform {
            // First perform the edits so that manifest paths are still valid.
            for (manifest_path, edits) in &manifest_edits {
                apply_edits(manifest_path, edits)?;
            }

            // Next, update the root manifest. Do this before moving directories because this relies
            // on the old directories existing.
            update_root_toml(workspace_root, &src_moves)
                .with_context(|| anyhow!("error while updating root toml at {}", workspace_root))?;

            // Finally, move directories into their new spots.
            // Rely on the fact that BTreeMap is sorted so that "foo" always shows up before
            // "foo/bar".
            // TODO: this would be better modeled as a trie.
            let mut done = HashSet::new();
            for (src_dir, package_move) in &src_moves {
                if src_dir.ancestors().any(|ancestor| done.contains(&ancestor)) {
                    // If we need to move both foo and foo/bar, and foo has already been moved,
                    // skip foo/bar.
                    continue;
                }
                let abs_src = workspace_root.join(src_dir);
                let abs_dest = workspace_root.join(&package_move.new_path);
                assert!(
                    !abs_dest.exists(),
                    "expected destination {} not to exist",
                    abs_dest
                );
                // fs::rename behaves differently on Unix and Windows if the destination exists.
                // But we don't expect it to, as the assertion above checks.
                fs::rename(&abs_src, &abs_dest).with_context(|| {
                    anyhow!("renaming {} to {} failed", src_dir, package_move.new_path)
                })?;
                done.insert(src_dir);
            }
        }

        Ok(())
    }
}

enum DestDir {
    Exists(Utf8PathBuf),
    Create(Utf8PathBuf),
}

impl DestDir {
    fn new(pkg_graph: &PackageGraph, dest_dir: &Utf8Path) -> Result<Self> {
        let workspace = pkg_graph.workspace();
        let workspace_root = workspace.root();

        match dest_dir.canonicalize() {
            Ok(dest_dir) => {
                if !dest_dir.is_dir() {
                    bail!("destination {} is not a directory", dest_dir.display());
                }

                // The destination directory exists.
                Ok(DestDir::Exists(
                    rel_path(&dest_dir, workspace_root)?.to_path_buf(),
                ))
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                // The destination directory doesn't exist and needs to be created.
                // Canonicalize the parent, then glue the last component to it.
                let last_component = dest_dir
                    .file_name()
                    .with_context(|| anyhow!("destination {} cannot end with ..", dest_dir))?;
                let parent = dest_dir
                    .parent()
                    .with_context(|| anyhow!("destination {} cannot be /", dest_dir))?;
                let parent = if parent.as_os_str() == "" {
                    Utf8Path::new(".")
                } else {
                    parent
                };

                let parent = canonicalize_dir(pkg_graph, parent)?;
                Ok(DestDir::Create(parent.join(last_component)))
            }
            Err(err) => {
                Err(err).with_context(|| anyhow!("reading destination {} failed", dest_dir))
            }
        }
    }

    fn is_create(&self) -> bool {
        match self {
            DestDir::Create(_) => true,
            DestDir::Exists(_) => false,
        }
    }

    fn join(&self, workspace_path: &Utf8Path, src_dir: &Utf8Path) -> Result<Utf8PathBuf> {
        // Consider e.g. workspace path = foo/bar, src dir = foo, dest dir = quux.
        let new_path = match self {
            DestDir::Exists(dest_dir) => {
                // quux exists, so the new path would be quux/foo/bar, not quux/bar. So look at the
                // src dir's parent.
                let trailing = workspace_path
                    .strip_prefix(src_dir.parent().expect("src dir should have a parent"))
                    .expect("workspace path is inside src dir");
                dest_dir.join(trailing)
            }
            DestDir::Create(dest_dir) => {
                // quux does not exist, so the new path would be quux/bar.
                let trailing = workspace_path
                    .strip_prefix(src_dir)
                    .expect("workspace path is inside src dir");
                dest_dir.join(trailing)
            }
        };

        // If the new path is inside (or the same as) the source directory, it's a problem.
        if new_path.starts_with(src_dir) {
            bail!("invalid move: {} -> {}", src_dir, new_path);
        }

        Ok(new_path)
    }
}

/// Return the workspace path for a given directory (relative to cwd).
fn canonicalize_dir(pkg_graph: &PackageGraph, path: impl AsRef<Utf8Path>) -> Result<Utf8PathBuf> {
    let workspace = pkg_graph.workspace();
    let workspace_root = workspace.root();

    let path = path.as_ref();
    let canonical_path = path
        .canonicalize()
        .with_context(|| anyhow!("reading path {} failed", path))?;
    if !canonical_path.is_dir() {
        bail!("path {} is not a directory", canonical_path.display());
    }

    Ok(rel_path(&canonical_path, workspace_root)?.to_path_buf())
}

fn rel_path<'a>(path: &'a Path, workspace_root: &Utf8Path) -> Result<&'a Utf8Path> {
    let rel_path = path.strip_prefix(workspace_root).with_context(|| {
        anyhow!(
            "path {} not in workspace root {}",
            path.display(),
            workspace_root
        )
    })?;
    Utf8Path::from_path(rel_path)
        .ok_or_else(|| anyhow!("rel path {} is invalid UTF-8", rel_path.display()))
}

/// Checks that the path is valid Unicode. If it is, returns a string, otherwise returns an error.
fn check_utf8_path(path: PathBuf) -> Result<Utf8PathBuf> {
    // TODO: remove this, rely on Utf8PathBuf instead!
    Utf8PathBuf::from_path_buf(path)
        .map_err(|non_unicode| anyhow!("path {} is not valid Unicode", non_unicode.display()))
}

fn moves_for<'g: 'a, 'a>(
    pkg_graph: &'g PackageGraph,
    src_dir: &'a Utf8Path,
    dest_dir: &'a DestDir,
) -> Result<Vec<(&'g Utf8Path, PackageMove<'g>)>> {
    // TODO: speed this up using a trie in guppy? Probably not that important.
    let workspace = pkg_graph.workspace();
    let workspace_root = workspace.root();
    // Ensure that the path refers to a package.
    let _package = workspace.member_by_path(src_dir)?;

    // Now look for all paths that start with the package.
    workspace
        .iter_by_path()
        .filter_map(move |(workspace_path, package)| {
            if workspace_path.starts_with(src_dir) {
                let pair = dest_dir.join(workspace_path, src_dir).and_then(|new_path| {
                    // Check that the new path doesn't exist already.
                    let abs_new_path = workspace_root.join(&new_path);
                    if abs_new_path.exists() {
                        bail!(
                            "attempted to move {} to {}, which already exists",
                            workspace_path,
                            new_path
                        );
                    }

                    // new_path can sometimes have a trailing slash -- remove it if it does.
                    let mut new_path = new_path.into_string();
                    if new_path.ends_with(MAIN_SEPARATOR) {
                        new_path.pop();
                    }
                    let new_path = new_path.into();

                    Ok((workspace_path, PackageMove { package, new_path }))
                });
                Some(pair)
            } else {
                None
            }
        })
        .collect()
}

#[derive(Clone, Debug)]
struct PackageMove<'g> {
    package: PackageMetadata<'g>,
    new_path: Utf8PathBuf,
}

#[derive(Clone, Debug)]
struct ManifestEdit<'g> {
    link: PackageLink<'g>,
    edit_path: Utf8PathBuf,
}

impl<'g> fmt::Display for ManifestEdit<'g> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "update {} to path {}",
            self.link.dep_name(),
            self.edit_path,
        )
    }
}

fn apply_edits(manifest_path: &Utf8Path, edits: &[ManifestEdit<'_>]) -> Result<()> {
    let mut document = read_toml(manifest_path)?;
    let table = document.as_table_mut();

    // This is annoying -- we need to grab a list of elements in target before processing it because
    // there's no iter_mut on toml_edit::Table (as of 0.1.5).
    let all_targets = match table.entry("target").as_table() {
        Some(target_tables) => target_tables
            .iter()
            .map(|(target, _)| target.to_string())
            .collect(),
        None => {
            // There's no 'target' section in the manifest.
            Vec::new()
        }
    };

    // Search through:
    // * dependencies, build-dependencies, dev-dependencies
    // * [target.'foo'.dependencies], .build-dependencies and .dev-dependencies
    for edit in edits {
        apply_edit(table, edit)
            .with_context(|| anyhow!("error while applying edits to {}", manifest_path))?;
        for target in &all_targets {
            let target_table = match &mut table["target"][target] {
                Item::Table(target_table) => target_table,
                _ => {
                    // Not a table, skip it.
                    continue;
                }
            };
            apply_edit(target_table, edit).with_context(|| {
                anyhow!(
                    "error while applying edits to {}, section [target.'{}']",
                    manifest_path,
                    target
                )
            })?;
        }
    }

    fs::write(manifest_path, document.to_string_in_original_order())
        .with_context(|| anyhow!("error while writing manifest {}", manifest_path))?;

    Ok(())
}

fn apply_edit(table: &mut Table, edit: &ManifestEdit<'_>) -> Result<()> {
    static SECTION_NAMES: &[&str] = &["dependencies", "build-dependencies", "dev-dependencies"];

    let dep_name = edit.link.dep_name();

    for section_name in SECTION_NAMES {
        let section = &mut table[section_name];
        let table = match section {
            Item::None => {
                // This section is empty -- skip it.
                continue;
            }
            Item::Table(table) => table,
            Item::Value(_) | Item::ArrayOfTables(_) => {
                bail!("section [{}] is not a table", section_name);
            }
        };

        match table.entry(dep_name) {
            Item::Table(dep_table) => {
                // The dep table should have a path entry.
                match dep_table.entry("path").as_value_mut() {
                    Some(value) => {
                        replace_decorated(value, edit.edit_path.as_str());
                    }
                    None => bail!(
                        "in section [{}], {}.path is not a string",
                        section_name,
                        dep_name
                    ),
                }
            }
            Item::Value(value) => match value.as_inline_table_mut() {
                Some(dep_table) => match dep_table.get_mut("path") {
                    Some(value) => {
                        replace_decorated(value, edit.edit_path.as_str());
                    }
                    None => bail!(
                        "in section [{}], {}.path is not a string",
                        section_name,
                        dep_name
                    ),
                },
                None => bail!(
                    "in section [{}], {} is not an inline table",
                    section_name,
                    dep_name
                ),
            },
            Item::None => continue,
            Item::ArrayOfTables(_) => {
                bail!("in section [{}], {} is not a table", section_name, dep_name)
            }
        }
    }

    Ok(())
}

fn update_root_toml(
    workspace_root: &Utf8Path,
    src_moves: &BTreeMap<&Utf8Path, PackageMove<'_>>,
) -> Result<()> {
    let root_manifest_path = workspace_root.join("Cargo.toml");
    let mut document = read_toml(&root_manifest_path)?;

    // Fix up paths in workspace.members or workspace.default-members.
    let workspace_table = match document.as_table_mut().entry("workspace") {
        Item::Table(workspace_table) => workspace_table,
        _ => bail!("[workspace] is not a table"),
    };

    static TO_UPDATE: &[&str] = &["members", "default-members"];

    for to_update in TO_UPDATE {
        let members = match workspace_table.entry(to_update) {
            Item::Value(members) => match members.as_array_mut() {
                Some(members) => members,
                None => bail!("in [workspace], {} is not an array", to_update),
            },
            Item::None => {
                // default-members may not always exist.
                continue;
            }
            _ => bail!("in [workspace], {} is not an array", to_update),
        };

        for idx in 0..members.len() {
            let member = members.get(idx).expect("valid idx");
            match member.as_str() {
                Some(path) => {
                    let abs_member_dir = workspace_root.join(path);
                    // The workspace path saved in the TOML may not be in canonical form.
                    let abs_member_dir = abs_member_dir.canonicalize().with_context(|| {
                        anyhow!(
                            "in [workspace] {}, error while canonicalizing path {}",
                            to_update,
                            path
                        )
                    })?;
                    // member dir is the canonical dir relative to the root.
                    let member_dir = rel_path(&abs_member_dir, workspace_root)?;

                    if let Some(package_move) = src_moves.get(member_dir) {
                        // This path was moved.
                        members
                            .replace(idx, package_move.new_path.as_str())
                            .expect("replacing string with string should work");
                    }
                }
                None => bail!("in [workspace], {} contains non-strings", to_update),
            }
        }
    }

    let mut out = fs::File::create(&root_manifest_path)
        .with_context(|| anyhow!("Error while opening {}", root_manifest_path))?;
    write!(out, "{}", document)?;

    Ok(())
}

fn read_toml(manifest_path: &Utf8Path) -> Result<Document> {
    let toml = fs::read_to_string(manifest_path)
        .with_context(|| anyhow!("error while reading manifest {}", manifest_path))?;
    toml.parse::<Document>()
        .with_context(|| anyhow!("error while parsing manifest {}", manifest_path))
}

/// Replace the value while retaining the decor.
fn replace_decorated(dest: &mut Value, new_value: impl Into<Value>) -> Value {
    let decor = dest.decor();
    let new_value = decorated(new_value.into(), decor.prefix(), decor.suffix());
    mem::replace(dest, new_value)
}
