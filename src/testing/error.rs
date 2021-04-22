//! Testing errors

use std::error;
use std::fmt;
use std::fmt::Display;
use std::sync::mpsc;

use rppal::gpio;

use crate::io;
use crate::sw;

/// Test-related error.
#[derive(Debug)]
pub enum Error {
    /// Testbed to device I/O error
    IO(io::Error),
    /// GPIO-related error
    GPIO(gpio::Error),
    /// Executor-observer thread communication error
    Comm(mpsc::RecvError),
    /// Error from spawning testbed threads.
    Threading(std::io::Error),
    /// Energy meter does not exist.
    NoSuchMeter(String),
    /// Device does not specify a platform when tests require one.
    DevicePlatform,
    /// Platform configuration not provided.
    NoPlatformConfig(String),
    /// No applications provided when tests require one.
    NoApplications,
    /// No such application defined.
    NoApplication(String),
    /// Error originating from interacting with software ([`sw::error::Error`]).
    Software(sw::error::Error),
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::IO(ref e) => Some(e),
            Error::GPIO(ref e) => Some(e),
            Error::Comm(ref e) => Some(e),
            Error::Threading(ref e) => Some(e),
            Error::Software(ref e) => Some(e),
            _ => None,
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

impl From<sw::error::Error> for Error {
    fn from(e: sw::error::Error) -> Error {
        Error::Software(e)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::IO(ref e) => write!(f, "I/O error: {}", e),
            Error::GPIO(ref e) => write!(f, "GPIO error while testing: {}", e),
            Error::Comm(ref e) => write!(f, "thread communication error: {}", e),
            Error::Threading(ref e) => write!(f, "thread spawning error: {}", e),
            Error::NoSuchMeter(ref id) => write!(f, "the meter '{}' does not exist", id),
            Error::DevicePlatform => write!(f, "device does does not specify a platform and it is necessary"),
            Error::NoPlatformConfig(ref name) => write!(f, "config for '{}' required but missing", name),
            Error::NoApplications => write!(f, "no applications defined but at least one expected"),
            Error::NoApplication(ref name) => write!(f, "no such application '{}' defined", name),
            Error::Software(ref e) => write!(f, "software interaction error: {}", e),
        }
    }
}
