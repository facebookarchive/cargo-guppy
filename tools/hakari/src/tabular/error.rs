/// Errors from parsing the table format string.
///
/// Returned by [`Table::new_safe()`].
///
/// [`Table::new_safe()`]: struct.Table.html#method.new_safe
#[derive(Debug, Clone)]
pub enum Error {
    /// Encountered a `{` character without matching `}`.
    ///
    /// The string is the contents of the column specifier, not including the `{` character.
    UnclosedColumnSpec(String),
    /// Did not understand the column specifiier.
    ///
    /// The string is the contents of the column specifier, not including the `{`
    /// and `}` characters.
    BadColumnSpec(String),
    /// Encountered a `}` character without a prior matching `{` character.
    UnexpectedRightBrace,
    /// Encountered a character unexpected inside a column specifier.
    UnexpectedCharacter(char),
}

impl ::std::error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::UnclosedColumnSpec(_) => "unclosed column specifier",
            Error::BadColumnSpec(_) => "bad format specifier",
            Error::UnexpectedRightBrace => "unexpected single '}' character",
            Error::UnexpectedCharacter(_) => "unexpected character in column specifier",
        }
    }
}

impl ::std::fmt::Display for Error {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match *self {
            Error::UnclosedColumnSpec(ref spec) => {
                write!(f, "unclosed column specifier: {:?}", spec)
            }
            Error::BadColumnSpec(ref spec) => write!(f, "bad format specifier: {:?}", spec),
            Error::UnexpectedRightBrace => f.write_str("unexpected single '}' character"),
            Error::UnexpectedCharacter(c) => {
                write!(f, "unexpected character in column specifier: {:?}", c)
            }
        }
    }
}

/// Type alias specializing `std::result::Result` with this crate’s [`Error`] enum.
///
/// [`Error`]: error.Error.html
pub type Result<T> = ::std::result::Result<T, Error>;
