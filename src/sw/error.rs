use std::convert::From;
use std::error;
use std::fmt;
use std::fmt::Display;
use std::process::Output;

#[derive(Debug)]
pub enum Error {
    // [`std::io`] error.
    IO(std::io::Error),
    /// Problem while working with external tools.
    Load(Output),
    /// Catch-all for other errors.
    Other(String),
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::IO(ref e) => Some(e),
            _ => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::IO(ref e) => write!(f, "I/O error: {}", e),
            Error::Load(ref output) => write!(f, "could not load software (status: {})", output.status),
            Error::Other(ref msg) => write!(f, "unexpected error: {}", msg),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::IO(e)
    }
}
