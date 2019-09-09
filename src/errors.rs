use std::error;
use std::fmt;
use std::io;

use Error::*;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    InvalidInput,
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Io(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Io(err) => write!(f, "{}", err),
            InvalidInput => write!(f, "Failed to read Cargo.lock file"),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Io(err) => Some(err),
            InvalidInput => None,
        }
    }
}
