/// A macro for building a [`Row`].
///
/// `row!(A, B, C)` is equivalent to
/// `Row::new().with_cell(A).with_cell(B).with_cell(C)`.
///
/// # Examples
///
/// ```
/// #[macro_use(row)]
/// extern crate tabular;
///
/// # fn main() {
/// let table = tabular::Table::new("{:>}  {:<}  {:<}")
///     .with_row(row!(34, "hello", true))
///     .with_row(row!(567, "goodbye", false));
///
/// assert_eq!( format!("\n{}", table),
///             r#"
///  34  hello    true
/// 567  goodbye  false
/// "# );
/// # }
/// ```
///
/// [`Row`]: struct.Row.html
#[macro_export]
macro_rules! row {
    ( $( $cell:expr ),* ) => {
        {
            let mut result = $crate::Row::new();
            $(
                result.add_cell($cell);
            )*
            result
        }
    };

    ( $( $cell:expr, )* ) => {
        row!( $( $cell ),* )
    };
}

/// A macro for building a [`Table`].
///
/// `table!(S, A, B, C)` is equivalent to
/// `Table::new(S).with_row(A).with_row(B).with_row(B)`.
///
/// # Examples
///
/// ```
/// #[macro_use(row, table)]
/// extern crate tabular;
///
/// # fn main() {
/// let table = table!("{:>}  {:<}  {:<}",
///                    row!(34, "hello", true),
///                    row!(567, "goodbye", false));
///
/// assert_eq!( format!("\n{}", table),
///             r#"
///  34  hello    true
/// 567  goodbye  false
/// "# );
/// # }
/// ```
///
/// [`Table`]: struct.Row.html
#[macro_export]
macro_rules! table {
    ( $row_spec:expr, $( $row:expr ),* ) => {
        {
            let mut result = $crate::Table::new($row_spec);
            $(
                result.add_row($row);
            )*
            result
        }
    };

    ( $row_spec:expr, $( $row:expr, )* ) => {
        table!($row_spec, $( row ),* )
    };
}
