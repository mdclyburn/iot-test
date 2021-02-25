use std::collections::HashMap;
use std::error;
use std::iter::IntoIterator;
use std::fmt;
use std::fmt::Display;

type Result<T> = std::result::Result<T, Error>;

#[derive(Copy, Clone, Debug)]
pub enum Error {
    UndefinedPin(u8),
}

impl error::Error for Error {  }

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::UndefinedPin(pin) => write!(f, "undefined pin used ({})", pin),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum IODirection {
    In,
    Out,
}

#[derive(Copy, Clone, Debug)]
pub enum Signal {
    Analog,
    Digital,
}

#[derive(Clone, Debug)]
pub struct Device {
    io: HashMap<u8, (IODirection, Signal)>,
}

impl Device {
    pub fn new<'a, T>(pin_map: T) -> Device where
        T: IntoIterator<Item = &'a (u8, (IODirection, Signal))> {
        Device {
            io: pin_map.into_iter().map(|x| *x).collect(),
        }
    }

    pub fn direction_of(&self, pin: u8) -> Result<IODirection> {
        self.io.get(&pin)
            .map(|&(dir, _sig)| dir )
            .ok_or(Error::UndefinedPin(pin))
    }

    pub fn signal_of(&self, pin: u8) -> Result<Signal> {
        self.io.get(&pin)
            .map(|&(_dir, sig)| sig)
            .ok_or(Error::UndefinedPin(pin))
    }
}
