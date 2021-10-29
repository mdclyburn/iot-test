//! Testing errors.

use std::error;
use std::fmt;
use std::fmt::Display;

use clockwise_common;
use clockwise_common::io;

use crate::sw;

/// Test-related error.
#[derive(Debug)]
pub enum Error {
    /// A problem occured while executing a test.
    Execution(clockwise_common::error::Error),
    /// Reset requested when [`clockwise_common::io::Mapping`] does not specify one.
    Reset(io::Error),
    /// Error originating from interacting with software ([`sw::error::Error`]).
    Software(sw::error::Error),
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::Execution(ref e) => Some(e),
            Error::Reset(ref e) => Some(e),
            Error::Software(ref e) => Some(e),
        }
    }
}

impl From<clockwise_common::error::Error> for Error {
    fn from(e: clockwise_common::error::Error) -> Self {
        Error::Execution(e)
    }
}

impl From<sw::error::Error> for Error {
    fn from(e: sw::error::Error) -> Error {
        Error::Software(e)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Execution(ref e) => write!(f, "test execution error: {}", e),
            Error::Reset(ref e) => write!(f, "failed to reset device: {}", e),
            Error::Software(ref e) => write!(f, "software interaction error: {}", e),
        }
    }
}
