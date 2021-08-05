//! Error-handling for working with external tooling.

use std::convert::From;
use std::error;
use std::fmt;
use std::fmt::Display;
use std::process::Output;

use super::Platform;

/// Software interaction errors.
#[derive(Debug)]
pub enum Error {
    /// A [`std::io`] error.
    IO(std::io::Error),
    /// Problem while working with external tools.
    Tool(Output),
    /// Catch-all for other errors.
    Other(String),
    /// Application not defined.
    App(String),
    /// Application not defined for platform.
    AppForPlatform(String, Platform),
    /// Unsupported operation.
    Unsupported,
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
            Error::Tool(ref output) => write!(f, "could not load software (status: {})", output.status),
            Error::Other(ref msg) => write!(f, "unexpected error: {}", msg),
            Error::App(ref name) => write!(f, "no '{}' app defined", name),
            Error::AppForPlatform(ref name, platform) => write!(f, "no '{}' app defined for {}", name, platform),
            Error::Unsupported => write!(f, "requested operation is implemented for the platform"),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::IO(e)
    }
}