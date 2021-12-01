use super::width_string::WidthString;

use std::fmt::{Debug, Display, Formatter};

/// Type for building a [`Table`] row.
///
/// Make a new one with [`Row::new()`], then add to it with [`Row::with_cell()`].
/// Or make a complete one with the [`row!()`] macro or [`Row::from_cells()`].
///
/// # Examples
///
/// ```
/// #[macro_use(row)]
/// extern crate tabular;
///
/// # fn main() {
/// let table = tabular::Table::new("{:>}  ({:<}) {:<}")
///     .with_row(row!(1, "I", "one"))
///     .with_row(row!(5, "V", "five"))
///     .with_row(row!(10, "X", "ten"))
///     .with_row(row!(50, "L", "fifty"))
///     .with_row(row!(100, "C", "one-hundred"));
///
/// assert_eq!( format!("\n{}", table),
///             r#"
///   1  (I) one
///   5  (V) five
///  10  (X) ten
///  50  (L) fifty
/// 100  (C) one-hundred
/// "# );
/// # }
/// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// struct DirEntry {
    ///     size:         usize,
    ///     is_directory: bool,
    ///     name:         String,
    /// }
    ///
    /// impl DirEntry {
    ///     fn to_row(&self) -> tabular::Row {
    ///         tabular::Row::new()
    ///             .with_cell(self.size)
    ///             .with_cell(if self.is_directory { "d" } else { "" })
    ///             .with_cell(&self.name)
    ///     }
    /// }
    /// ```
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

    /// Builds a row from an iterator over strings.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tabular::*;
    /// use std::fmt::Display;
    ///
    /// struct Matrix<'a, T: 'a> {
    ///     width:  usize,
    ///     height: usize,
    ///     data:   &'a [T],
    /// }
    ///
    /// impl<'a, T: Display> Display for Matrix<'a, T> {
    ///     fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    ///         let ncols = self.width;
    ///         let row_spec: String =
    ///              std::iter::repeat("{:>} ".chars()).take(ncols).flat_map(|x| x).collect();
    ///
    ///         let mut table = Table::new(row_spec.trim_end());
    ///
    ///         for row_index in 0 .. self.height {
    ///             table.add_row(Row::from_cells(
    ///                 self.data[row_index * ncols ..]
    ///                     .iter().take(ncols)
    ///                     .map(|elt: &T| elt.to_string())));
    ///         }
    ///
    ///         write!(f, "{}", table)
    ///     }
    /// }
    ///
    /// print!("{}", Matrix {
    ///     width:   3,
    ///     height:  2,
    ///     data:    &[1, 23, 456, 7890, 12345, 678901],
    /// });
    /// ```
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
    ///
    /// # Examples
    ///
    /// It's probably not actually useful, because you are unlikely to come
    /// upon a row whose size you don't already know. But it's useful for stating
    /// [`Table::add_row`]'s invariant.
    ///
    /// ```
    /// # use tabular::*;
    /// # use std::fmt::Display;
    /// fn print_ragged_matrix<T: Display>(matrix: &[&[T]]) {
    ///    let ncols = matrix.iter().map(|row| row.len()).max().unwrap_or(0);
    ///
    ///    let mut row_spec = String::with_capacity(5 * ncols);
    ///    for _ in 0 .. ncols {
    ///        row_spec.push_str("{:>} ");
    ///    }
    ///
    ///    let mut table = Table::new(row_spec.trim_end());
    ///
    ///    for row in matrix {
    ///        let mut table_row = Row::from_cells(row.iter().map(ToString::to_string));
    ///
    ///        // Don't remember how to count or subtract but I'll get there eventually.
    ///        while table_row.len() < table.column_count() {
    ///            table_row.add_cell("");
    ///        }
    ///    }
    ///
    ///    print!("{}", table);
    /// }
    ///
    /// print_ragged_matrix(&[&[1, 2, 3, 4, 5], &[12, 23, 34], &[123, 234], &[1234]]);
    /// ```
    ///
    /// [`Table::add_row`]: struct.Table.html#method.add_row
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
