use super::{
    column_spec::{parse_row_spec, row_spec_to_string, ColumnSpec},
    error::Result,
    row::{InternalRow, Row},
    width_string::WidthString,
};

use std::fmt::{Debug, Display, Formatter};

/// Builder type for constructing a formatted table.
///
/// Construct this with [`Table::new()`] or [`Table::new_safe()`]. Then add rows
/// to it with [`Table::add_row()`] and [`Table::add_heading()`].
///
/// [`Table::new_safe()`]: struct.Table.html#method.new_safe
/// [`Table::new()`]: struct.Table.html#method.new
/// [`Table::add_row()`]: struct.Table.html#method.add_row
/// [`Table::add_heading()`]: struct.Table.html#method.add_heading
#[derive(Clone)]
pub struct Table {
    n_columns: usize,
    format: Vec<ColumnSpec>,
    rows: Vec<InternalRow>,
    column_widths: Vec<usize>,
    line_end: String,
}

const DEFAULT_LINE_END: &str = "\n";

impl Table {
    /// Constructs a new table with the format of each row specified by `row_spec`.
    ///
    /// Unlike `format!` and friends, `row_spec` is processed dynamically, but it uses a small
    /// subset of the syntax to determine how columns are laid out. In particular:
    ///
    ///   - `{:<}` produces a left-aligned column.
    ///
    ///   - `{:^}` produces a centered column.
    ///
    ///   - `{:>}` produces a right-aligned column.
    ///
    ///   - `{{` produces a literal `{` character.
    ///
    ///   - `}}` produces a literal `}` character.
    ///
    ///   - Any other appearances of `{` or `}` are errors.
    ///
    ///   - Everything else stands for itself.
    pub fn new(row_spec: &str) -> Self {
        Self::new_safe(row_spec)
            .unwrap_or_else(|e: super::error::Error| panic!("tabular::Table::new: {}", e))
    }

    /// Like [`new`], but returns a [`Result`] instead of panicking if parsing `row_spec` fails.
    ///
    /// [`new`]: #method.new
    /// [`Result`]: type.Result.html
    pub fn new_safe(row_spec: &str) -> Result<Self> {
        let (format, n_columns) = parse_row_spec(row_spec)?;

        Ok(Table {
            n_columns,
            format,
            rows: vec![],
            column_widths: vec![0; n_columns],
            line_end: DEFAULT_LINE_END.to_owned(),
        })
    }

    /// The number of columns in the table.
    pub fn column_count(&self) -> usize {
        // ^^^^^^^^^^^^ What’s a better name for this?
        self.n_columns
    }

    /// Adds a pre-formatted row that spans all columns.
    ///
    /// A heading does not interact with the formatting of rows made of cells.
    /// This is like `\intertext` in LaTeX, not like `<head>` or `<th>` in HTML.
    pub fn add_heading<S: Into<String>>(&mut self, heading: S) -> &mut Self {
        self.rows.push(InternalRow::Heading(heading.into()));
        self
    }

    /// Convenience function for calling [`add_heading`].
    ///
    /// [`add_heading`]: #method.add_heading
    pub fn with_heading<S: Into<String>>(mut self, heading: S) -> Self {
        self.add_heading(heading);
        self
    }

    /// Adds a row made up of cells.
    ///
    /// When printed, each cell will be padded to the size of its column, which is the maximum of
    /// the width of its cells.
    ///
    /// # Panics
    ///
    /// If `self.`[`column_count()`]` != row.`[`len()`].
    ///
    /// [`column_count()`]: #method.column_count
    /// [`len()`]: struct.Row.html#method.len
    pub fn add_row(&mut self, row: Row) -> &mut Self {
        let cells = row.0;

        assert_eq!(cells.len(), self.n_columns);

        for (width, s) in self.column_widths.iter_mut().zip(cells.iter()) {
            *width = ::std::cmp::max(*width, s.width());
        }

        self.rows.push(InternalRow::Cells(cells));
        self
    }

    /// Convenience function for calling [`add_row`].
    ///
    /// # Panics
    ///
    /// The same as [`add_row`].
    ///
    /// [`add_row`]: #method.add_row
    pub fn with_row(mut self, row: Row) -> Self {
        self.add_row(row);
        self
    }

    /// Sets the string to output at the end of every line.
    ///
    /// By default this is `"\n"` on all platforms, like `println!`.
    pub fn set_line_end<S: Into<String>>(mut self, line_end: S) -> Self {
        self.line_end = line_end.into();
        self
    }
}

impl Debug for Table {
    // This method allocates in two places:
    //   - row_spec_to_string
    //   - row.clone()
    // It doesn't need to do either.
    fn fmt(&self, f: &mut Formatter) -> ::std::fmt::Result {
        write!(f, "Table::new({:?})", row_spec_to_string(&self.format))?;

        if self.line_end != DEFAULT_LINE_END {
            write!(f, ".set_line_end({:?})", self.line_end)?;
        }

        for row in &self.rows {
            match *row {
                InternalRow::Cells(ref row) => write!(f, ".with_row({:?})", Row(row.clone()))?,

                InternalRow::Heading(ref heading) => write!(f, ".with_heading({:?})", heading)?,
            }
        }

        Ok(())
    }
}

impl Display for Table {
    fn fmt(&self, f: &mut Formatter) -> ::std::fmt::Result {
        use super::column_spec::{Alignment::*, ColumnSpec::*};

        let max_column_width = self.column_widths.iter().cloned().max().unwrap_or(0);
        let mut spaces = String::with_capacity(max_column_width);
        for _ in 0..max_column_width {
            spaces.push(' ');
        }

        let mt_width_string = WidthString::default();
        let is_not_last = |field_index| field_index + 1 < self.format.len();

        for row in &self.rows {
            match *row {
                InternalRow::Cells(ref cells) => {
                    let mut cw_iter = self.column_widths.iter().cloned();
                    let mut row_iter = cells.iter();

                    for field_index in 0..self.format.len() {
                        match self.format[field_index] {
                            Align(alignment) => {
                                let cw = cw_iter.next().unwrap();
                                let ws = row_iter.next().unwrap_or(&mt_width_string);
                                let needed = cw - ws.width();
                                let padding = &spaces[..needed];

                                match alignment {
                                    Left => {
                                        f.write_str(ws.as_str())?;
                                        if is_not_last(field_index) {
                                            f.write_str(padding)?;
                                        }
                                    }

                                    Center => {
                                        let (before, after) = padding.split_at(needed / 2);
                                        f.write_str(before)?;
                                        f.write_str(ws.as_str())?;
                                        if is_not_last(field_index) {
                                            f.write_str(after)?;
                                        }
                                    }

                                    Right => {
                                        f.write_str(padding)?;
                                        f.write_str(ws.as_str())?;
                                    }
                                }
                            }

                            Literal(ref s) => f.write_str(s)?,
                        }
                    }
                }

                InternalRow::Heading(ref s) => {
                    f.write_str(s)?;
                }
            }
            f.write_str(&self.line_end)?;
        }

        Ok(())
    }
}
