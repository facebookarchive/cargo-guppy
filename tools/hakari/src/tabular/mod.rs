//! Builds plain, automatically-aligned tables of monospaced text.
//! This is basically what you want if you are implementing `ls`.
//!
//! # Example
//!
//! ```
//! use tabular::{Table, Row};
//! use std::path::Path;
//!
//! fn ls(dir: &Path) -> ::std::io::Result<()> {
//!     let mut table = Table::new("{:>}  {:<}{:<}  {:<}");
//!     for entry_result in ::std::fs::read_dir(dir)? {
//!         let entry    = entry_result?;
//!         let metadata = entry.metadata()?;
//!
//!         table.add_row(Row::new()
//!              .with_cell(metadata.len())
//!              .with_cell(if metadata.permissions().readonly() {"r"} else {""})
//!              .with_cell(if metadata.is_dir() {"d"} else {""})
//!              .with_cell(entry.path().display()));
//!     }
//!
//!     print!("{}", table);
//!
//!     Ok(())
//! }
//!
//! ls(Path::new(&"target")).unwrap();
//! ```
//!
//! produces something like
//!
//! ```text
//! 1198     target/.rustc_info.json
//! 1120  d  target/doc
//!  192  d  target/package
//! 1056  d  target/debug
//! ```
//!
//! # Other features
//!
//!   - The [`Table::with_header`] and [`Table::add_header`] methods add
//!     lines that span all columns.
//!
//!   - The [`row!`] macro builds a row with a fixed number of columns
//!     using less syntax.
//!
//!   - The [`Table::set_line_end`] method allows changing the line ending
//!     to include a carriage return (or whatever you want).
//!
//! # Usage
//!
//! It's on [crates.io](https://crates.io/crates/tabular), so you can add
//!
//! ```toml
//! [dependencies]
//! tabular = "0.1.4"
//! ```
//!
//! to your `Cargo.toml`.
//!
//!
//! Feature `unicode-width` is enabled be default; it depends on the
//! [unicode-width](https://crates.io/crates/unicode-width) crate. You can turn
//! it off with:
//!
//! ```toml
//! [dependencies]
//! tabular = { version = "0.1.4", default-features = false }
//! ```
//!
//! Note that without `unicode-width`, alignment will be based on the count of the
//! `std::str::Chars` iterator.
//!
//! This crate supports Rust version 1.31.0 and later.
//!
//! # See also
//!
//! You may also want:
//!
//! - [text-tables](https://crates.io/crates/text-tables) – This is more automatic
//! than tabular. You give it an array of arrays, it renders a nice table with borders.
//! Tabular doesn't do borders.
//!
//! - [prettytable](https://crates.io/crates/prettytable-rs) — This has an API more
//! similar to tabular’s in terms of building a table, but it does a lot more, including,
//! color, borders, and CSV import.
//!
//! [`row!`]: macro.row.html
//! [`Row`]: struct.Row.html
//! [`Table`]: struct.Table.html
//! [`Table::add_header`]: struct.Table.html#method.add_header
//! [`Table::add_row`]: struct.Table.html#method.add_row
//! [`Table::new`]: struct.Table.html#method.new
//! [`Table::set_line_end`]: struct.Table.html#method.set_line_end
//! [`Table::with_row`]: struct.Table.html#method.with_row
//! [`Table::with_header`]: struct.Table.html#method.with_header

#![warn(missing_docs)]
#![allow(dead_code)]

#[cfg(feature = "unicode-width")]
extern crate unicode_width;

mod column_spec;
mod error;
mod macros;
mod row;
mod table;
mod width_string;

pub use self::{
    error::{Error, Result},
    row::Row,
    table::Table,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alignment() {
        let mut table = Table::new("{:>}  ({:<}) {:<}");
        table
            .add_row(Row::new().with_cell(1).with_cell("I").with_cell("one"))
            .add_row(Row::new().with_cell(5).with_cell("V").with_cell("five"))
            .add_row(Row::new().with_cell(10).with_cell("X").with_cell("ten"))
            .add_row(Row::new().with_cell(50).with_cell("L").with_cell("fifty"))
            .add_row(
                Row::new()
                    .with_cell(100)
                    .with_cell("C")
                    .with_cell("one-hundred"),
            );
        assert_eq!(
            format!("\n{}", table),
            r#"
  1  (I) one
  5  (V) five
 10  (X) ten
 50  (L) fifty
100  (C) one-hundred
"#
        );
    }

    #[test]
    fn heading() {
        let _row = Row::from_cells(vec!["a", "b", "c"]);
        //        eprintln!("{:?}", _row);

        let table = Table::new("{:<} {:<} {:>}")
            .with_row(Row::from_cells(vec!["a", "b", "d"]))
            .with_heading("This is my table")
            .with_row(Row::from_cells(vec!["ab", "bc", "cd"]));

        //        eprintln!("\n\n{:?}\n\n", table);

        assert_eq!(
            format!("\n{}", table),
            r#"
a  b   d
This is my table
ab bc cd
"#
        );
    }

    #[test]
    fn centering() {
        let table = Table::new("{:<} {:^} {:>}")
            .with_row(Row::from_cells(vec!["a", "b", "c"]))
            .with_row(Row::from_cells(vec!["a", "bc", "d"]))
            .with_row(Row::from_cells(vec!["a", "bcd", "e"]))
            .with_row(Row::from_cells(vec!["a", "bcde", "f"]))
            .with_row(Row::from_cells(vec!["a", "bcdef", "g"]));

        assert_eq!(
            format!("\n{}", table),
            r#"
a   b   c
a  bc   d
a  bcd  e
a bcde  f
a bcdef g
"#
        );
    }

    #[test]
    fn temporary() {}
}
