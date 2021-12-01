use super::width_string::WidthString;

use std::fmt::{Debug, Display, Formatter};

/// Type for building a [`Table`] row.
///
/// Make a new one with [`Row::new()`], then add to it with [`Row::with_cell()`].
/// Or make a complete one with the [`row!()`] macro or [`Row::from_cells()`].
///
/// [`Table`]: struct.Table.html
/// [`row!()`]: macro.row.html
/// [`Row::new()`]: struct.Row.html#method.new
/// [`Row::from_cells()`]: struct.Row.html#method.from_cells
/// [`Row::with_cell()`]: struct.Row.html#method.with_cell
#[derive(Clone, Default)]
pub struct Row(pub(crate) Vec<WidthString>);

impl Row {
    /// Makes a new, empty table row.
    pub fn new() -> Self {
        Row(Vec::new())
    }

    /// Adds a cell to this table row.
    pub fn with_cell<S: Display>(mut self, value: S) -> Self {
        self.add_cell(value);
        self
    }

    /// Adds a cell to this table row.
    ///
    /// This performs the same work as [`with_cell`], but it's is convenient for adding cells in
    /// a loop without having to reassign the row each time. See the example for [`len`].
    ///
    /// [`with_cell`]: #method.with_cell
    /// [`len`]: #method.len
    pub fn add_cell<S: Display>(&mut self, value: S) -> &mut Self {
        self.0.push(WidthString::new(value));
        self
    }

    /// Adds a cell to this table row after stripping ANSI escape sequences.
    ///
    /// If the table is being printed out to a terminal that supports ANSI escape sequences,
    /// cell widths need to account for that.
    pub fn with_ansi_cell<S: Display>(mut self, value: S) -> Self {
        self.add_ansi_cell(value);
        self
    }

    /// Adds a cell to this table row with a custom width.
    ///
    /// Similar to [`with_ansi_cell`], except it returns `&mut Self` rather than `Self`.
    pub fn add_ansi_cell<S: Display>(&mut self, value: S) -> &mut Self {
        self.0.push(WidthString::new_ansi(value));
        self
    }

    /// Adds a cell to this table row with a custom width.
    ///
    /// Cell widths are normally calculated by looking at the string. In some cases, such as if the
    /// string contains escape sequences of some sort, users may wish to specify a specific width
    /// to use for a custom cell.
    pub fn with_custom_width_cell<S: Display>(mut self, value: S, width: usize) -> Self {
        self.add_custom_width_cell(value, width);
        self
    }

    /// Adds a cell to this table row with a custom width.
    ///
    /// Similar to [`with_custom_width_cell`], except it returns `&mut Self` rather than `Self`.
    pub fn add_custom_width_cell<S: Display>(&mut self, value: S, width: usize) -> &mut Self {
        self.0.push(WidthString::custom_width(value, width));
        self
    }

    /// Builds a row from an iterator over strings.
    pub fn from_cells<S, I>(values: I) -> Self
    where
        S: Into<String>,
        I: IntoIterator<Item = S>,
    {
        Row(values
            .into_iter()
            .map(Into::into)
            .map(WidthString::new)
            .collect())
    }

    /// The number of cells in this row.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the row is empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Debug for Row {
    fn fmt(&self, f: &mut Formatter) -> ::std::fmt::Result {
        write!(f, "Row::from_cells(vec!{:?})", self.0)
    }
}

#[derive(Clone, Debug)]
pub enum InternalRow {
    Cells(Vec<WidthString>),
    Heading(String),
}
