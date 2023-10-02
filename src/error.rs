//! Common error type.

use core::fmt::{Display, Formatter};

/// Common error type.
#[derive(Debug)]
pub enum Error {
    /// Invalid data.
    InvalidData(String),
    /// Error from `flechasdb`.
    FlechasDBError(flechasdb::error::Error),
    /// IO error.
    IOError(std::io::Error),
}

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::InvalidData(s) => write!(f, "Invalid data: {}", s),
            Error::FlechasDBError(e) => write!(f, "FlechasDB error: {}", e),
            Error::IOError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl From<flechasdb::error::Error> for Error {
    fn from(e: flechasdb::error::Error) -> Self {
        Error::FlechasDBError(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::IOError(e)
    }
}
