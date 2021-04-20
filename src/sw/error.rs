use std::error;
use std::fmt;
use std::fmt::Display;

#[derive(Debug)]
pub enum Error {
    /// Problem loading software to device.
    Load(String)
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            _ => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Load(ref msg) => write!(f, "could not load software: {}", msg)
        }
    }
}
