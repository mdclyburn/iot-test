use std::error;
use std::fmt;
use std::fmt::Display;

use crate::io;

#[derive(Debug)]
pub enum Error {
    IO(io::Error),
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::IO(ref e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::IO(e)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::IO(ref e) => write!(f, "I/O error: {}", e),
        }
    }
}
