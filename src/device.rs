/*!
Device definition-related items.

To run a test against a device, it must be possible to know what the device under test is capable of.
This module provides code related to expressing how the device can accept input or provide output.

See [`Device`] for more information.
!*/

use std::collections::HashMap;
use std::error;
use std::iter::IntoIterator;
use std::fmt;
use std::fmt::Display;

type Result<T> = std::result::Result<T, Error>;

/// Device-related errors
#[derive(Copy, Clone, Debug)]
pub enum Error {
    /// A provided pin was not defined.
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

/// I/O pin direction from the perspective of the device under test
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum IODirection {
    /// Input to the device
    In,
    /// Output from the device
    Out,
}

/// Signal type
#[derive(Copy, Clone, Debug)]
pub enum Signal {
    /// Analog signal
    Analog,
    /// Digital signal
    Digital,
}

/// Properties about a device under test
#[derive(Clone, Debug)]
pub struct Device {
    io: HashMap<u8, (IODirection, Signal)>,
}

impl Device {
    /*! Define a new device

    A device under test has a defined set of inputs and outputs.
    Each I/O has a signal type that it emits or accepts.

    # Examples

    ```
    Device::new(&[
       (2, (IODirection::In, Signal::Digital)),
       (3, (IODirection::Out, Signal::Digital)),
    ]);
    ```
    !*/
    pub fn new<'a, T>(pin_map: T) -> Device where
        T: IntoIterator<Item = &'a (u8, (IODirection, Signal))> {
        Device {
            io: pin_map.into_iter().map(|x| *x).collect(),
        }
    }

    /// Returns the direction of the pin.
    ///
    /// Returns an error if the pin is not defined.
    pub fn direction_of(&self, pin: u8) -> Result<IODirection> {
        self.io.get(&pin)
            .map(|&(dir, _sig)| dir )
            .ok_or(Error::UndefinedPin(pin))
    }

    /// Returns the signal of the pin.
    ///
    /// Returns an error if the pin is not defined.
    pub fn signal_of(&self, pin: u8) -> Result<Signal> {
        self.io.get(&pin)
            .map(|&(_dir, sig)| sig)
            .ok_or(Error::UndefinedPin(pin))
    }
}
