//! Testing errors

use std::error;
use std::fmt;
use std::fmt::Display;
use std::sync::mpsc;

use rppal::gpio;

use crate::io;

/// Test-related error.
#[derive(Debug)]
pub enum Error {
    /// Testbed to device I/O error
    IO(io::Error),
    /// GPIO-related error
    GPIO(gpio::Error),
    /// Executor-observer thread communication error
    Comm(mpsc::RecvError),
    /// Observer thread spawning error
    Observer(std::io::Error),
    /// Metering thread spawning error
    Meter(std::io::Error),
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::IO(ref e) => Some(e),
            Error::GPIO(ref e) => Some(e),
            Error::Comm(ref e) => Some(e),
            Error::Observer(ref e) => Some(e),
            Error::Meter(ref e) => Some(e),
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::IO(e)
    }
}

impl From<gpio::Error> for Error {
    fn from(e: gpio::Error) -> Self {
        Error::GPIO(e)
    }
}

impl From<mpsc::RecvError> for Error {
    fn from(e: mpsc::RecvError) -> Self {
        Error::Comm(e)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::IO(ref e) => write!(f, "I/O error: {}", e),
            Error::GPIO(ref e) => write!(f, "GPIO error while testing: {}", e),
            Error::Comm(ref e) => write!(f, "Thread communication error: {}", e),
            Error::Observer(ref e) => write!(f, "Could not start observer thread: {}", e),
            Error::Meter(ref e) => write!(f, "Could not start metering thread: {}", e),
        }
    }
}
