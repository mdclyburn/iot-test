//! Error-handling for working with external tooling.

use std::convert::From;
use std::error;
use std::fmt;
use std::fmt::Display;
use std::process::Output;

use super::Platform;

/// Erros that occur while interacting with software that facilitates testing or runs on DUTs.
#[derive(Debug)]
pub enum SoftwareError {
    /// A [`std::io`] error.
    IO(std::io::Error),
    /// Problem while working with external tools.
    Tool(Output),
    /// Application not defined for platform.
    AppForPlatform(String, Platform),
    /// Unsupported operation.
    Unsupported,
}

impl error::Error for SoftwareError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            SoftwareError::IO(ref e) => Some(e),
            _ => None,
        }
    }
}

impl Display for SoftwareError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use SoftwareError::*;
        match self {
            IO(ref e) => write!(f, "I/O error: {}", e),
            Tool(ref output) => write!(f, "could not load software (status: {})", output.status),
            AppForPlatform(ref name, platform) => write!(f, "no '{}' app defined for {}", name, platform),
            Unsupported => write!(f, "requested operation is implemented for the platform"),
        }
    }
}

impl From<std::io::Error> for SoftwareError {
    fn from(e: std::io::Error) -> Self {
        SoftwareError::IO(e)
    }
}
