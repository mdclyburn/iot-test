//! Testbed input-related errors.

use std::convert::From;
use std::error;
use std::fmt;
use std::fmt::Display;

use clockwise_common::io as hw_io;

/// Errors from creating testbeds and tests from provided input.
#[derive(Debug)]
pub enum Error {
    /// A problem with a configuration provided to a driver.
    Driver(String),
    /// Input format error with provided input.
    Format(String),
    /// Testbed hardware I/O error.
    Hardware(hw_io::Error),
    /// I/O error.
    IO(std::io::Error),
    /// Provided input is not supported.
    Unsupported,
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use Error::*;
        match self {
            Hardware(ref e) => Some(e),
            IO(ref e) => Some(e),
            _ => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Error::*;
        match self {
            Driver(ref msg) => write!(f, "driver initialization error: {}", msg),
            Format(ref msg) => write!(f, "input format error: {}", msg),
            Hardware(ref _e) => write!(f, "hardware I/O error"),
            IO(ref _e) => write!(f, "I/O error"),
            Unsupported => write!(f, "part of input unsupported"),
        }
    }
}

impl From<hw_io::Error> for Error {
    fn from(e: hw_io::Error) -> Self {
        Error::Hardware(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::IO(e)
    }
}
