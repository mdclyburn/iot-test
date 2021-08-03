//! Testing errors.

use std::error;
use std::fmt;
use std::fmt::Display;
use std::sync::mpsc;

use flexbed_common::io;
use flexbed_common::sw;
use rppal::gpio;
use rppal::uart;


/// Test-related error.
#[derive(Debug)]
pub enum Error {
    /// Testbed to device I/O error.
    IO(io::Error),
    /// GPIO-related error.
    GPIO(gpio::Error),
    /// Executor-observer thread communication error.
    Comm(mpsc::RecvError),
    /// Error from spawning testbed threads.
    Threading(std::io::Error),
    /// Energy meter does not exist.
    NoSuchMeter(String),
    /// Reset requested when [`Mapping`] does not specify one.
    Reset(io::Error),
    /// Error originating from interacting with software ([`sw::error::Error`]).
    Software(sw::error::Error),
    /// Error configuring UART hardware.
    UART(uart::Error),
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::IO(ref e) => Some(e),
            Error::GPIO(ref e) => Some(e),
            Error::Comm(ref e) => Some(e),
            Error::Reset(ref e) => Some(e),
            Error::Software(ref e) => Some(e),
            Error::Threading(ref e) => Some(e),
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

impl From<uart::Error> for Error {
    fn from(e: uart::Error) -> Error {
        Error::UART(e)
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
            Error::Reset(ref e) => write!(f, "failed to reset device: {}", e),
            Error::Software(ref e) => write!(f, "software interaction error: {}", e),
            Error::UART(ref e) => write!(f, "UART configuration error: {}", e),
        }
    }
}
