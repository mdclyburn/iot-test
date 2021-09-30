//! Testing errors.

use std::error;
use std::fmt;
use std::fmt::Display;

use rppal::gpio;
use rppal::uart;

use crate::io;

/// Test-related error.
#[derive(Debug)]
pub enum Error {
    /// GPIO-related error.
    GPIO(gpio::Error),
    /// Testbed to device I/O error.
    IO(io::Error),
    /// Energy meter does not exist.
    NoSuchMeter(String),
    /// Invalid test protocol data received.
    Protocol,
    /// Reset requested when [`io::Mapping`] does not specify one.
    Reset(io::Error),
    /// Error configuring UART hardware.
    UART(uart::Error),
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::GPIO(ref e) => Some(e),
            Error::IO(ref e) => Some(e),
            Error::Protocol => None,
            Error::Reset(ref e) => Some(e),
            Error::UART(ref e) => Some(e),
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

impl From<uart::Error> for Error {
    fn from(e: uart::Error) -> Error {
        Error::UART(e)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::GPIO(ref e) => write!(f, "GPIO error while testing: {}", e),
            Error::IO(ref e) => write!(f, "I/O error: {}", e),
            Error::NoSuchMeter(ref id) => write!(f, "the meter '{}' does not exist", id),
            Error::Protocol => write!(f, "testbed/DUT test protocol mismatch"),
            Error::Reset(ref e) => write!(f, "failed to reset device: {}", e),
            Error::UART(ref e) => write!(f, "UART configuration error: {}", e),
        }
    }
}
