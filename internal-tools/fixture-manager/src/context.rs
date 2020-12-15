// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{bail, Result};
use fixtures::json::JsonFixture;
use std::path::{Path, PathBuf};

pub trait ContextImpl<'g> {
    type IterArgs;
    type IterItem;
    type Existing;

    fn dir_name(fixture: &'g JsonFixture) -> PathBuf;
    fn file_name(fixture: &'g JsonFixture, item: &Self::IterItem) -> String;

    fn iter(
        fixture: &'g JsonFixture,
        args: &Self::IterArgs,
    ) -> Box<dyn Iterator<Item = Self::IterItem> + 'g>;

    fn parse_existing(path: &Path, contents: String) -> Result<Self::Existing>;
    fn is_changed(item: &Self::IterItem, existing: &Self::Existing) -> bool;
    fn diff(
        fixture: &'g JsonFixture,
        item: &Self::IterItem,
        existing: Option<&Self::Existing>,
    ) -> String;

    fn write_to_string(
        fixture: &'g JsonFixture,
        item: &Self::IterItem,
        out: &mut String,
    ) -> Result<()>;
}

pub trait ContextDiff<'a> {}

pub struct GenerateContext<'g, T: ContextImpl<'g>> {
    fixture: &'g JsonFixture,
    skip_existing: bool,
    file_template: PathBuf,
    iter: Box<dyn Iterator<Item = T::IterItem> + 'g>,
}

impl<'g, T: ContextImpl<'g>> GenerateContext<'g, T> {
    pub fn new(fixture: &'g JsonFixture, args: &T::IterArgs, skip_existing: bool) -> Result<Self> {
        let mut file_template = T::dir_name(fixture);
        file_template.push("REPLACE_THIS_FILE_NAME");

        std::fs::create_dir_all(
            file_template
                .parent()
                .expect("file_template should not return root or prefix"),
        )?;
        let iter = T::iter(fixture, args);
        Ok(Self {
            fixture,
            skip_existing,
            file_template,
            iter,
        })
    }
}

impl<'g, T: ContextImpl<'g>> Iterator for GenerateContext<'g, T> {
    type Item = Result<ContextItem<'g, T>>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.iter.next()?;

        let mut path = self.file_template.clone();
        path.set_file_name(T::file_name(self.fixture, &item));
        let existing = if self.skip_existing {
            // In force mode, treat the on-disk contents as missing.
            None
        } else {
            match read_contents(&path) {
                Ok(Some(contents)) => match T::parse_existing(&path, contents) {
                    Ok(existing) => Some(existing),
                    Err(err) => return Some(Err(err)),
                },
                Ok(None) => None,
                Err(err) => return Some(Err(err)),
            }
        };

        Some(Ok(ContextItem {
            fixture: self.fixture,
            path,
            item,
            existing,
        }))
    }
}

pub struct ContextItem<'g, T: ContextImpl<'g>> {
    fixture: &'g JsonFixture,
    path: PathBuf,
    item: T::IterItem,
    existing: Option<T::Existing>,
}

impl<'g, T: ContextImpl<'g>> ContextItem<'g, T> {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn is_changed(&self) -> bool {
        match &self.existing {
            Some(existing) => T::is_changed(&self.item, existing),
            None => {
                // File doesn't exist: treat as changed.
                true
            }
        }
    }

    pub fn diff(&self) -> String {
        T::diff(self.fixture, &self.item, self.existing.as_ref())
    }

    pub fn write_to_path(&self) -> Result<()> {
        let mut out = String::new();

        if let Err(err) = T::write_to_string(self.fixture, &self.item, &mut out) {
            eprintln!("** Partially generated output:\n{}", out);
            bail!(
                "Error while writing to string: {}\n\nPartially generated output:\n{}",
                err,
                out
            );
        }

        Ok(std::fs::write(&self.path, &out)?)
    }
}

fn read_contents(file: &Path) -> Result<Option<String>> {
    let contents = match std::fs::read_to_string(file) {
        Ok(data) => data,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                // Don't fail if the file wasn't found.
                return Ok(None);
            }
            return Err(err.into());
        }
    };

    Ok(Some(contents))
}
