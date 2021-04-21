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

use crate::comm::{Direction, Class as SignalClass};
use crate::sw::Platform;

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

/// Properties about a device under test
#[derive(Clone, Debug)]
pub struct Device {
    platform: Option<Platform>,
    io: HashMap<u8, (Direction, SignalClass)>,
}

impl Device {
    /*! Define a new device

    A device under test has a defined set of inputs and outputs.
    Each I/O has a signal type that it emits or accepts.

    # Examples

    ```
    Device::new(&[
       (2, (Direction::In, SignalClass::Digital)),
       (3, (Direction::Out, SignalClass::Digital)),
    ]);
    ```
    !*/
    pub fn new<'a, T>(platform: Option<Platform>, pin_map: T) -> Device where
        T: IntoIterator<Item = &'a (u8, (Direction, SignalClass))> {
        Device {
            platform,
            io: pin_map.into_iter().map(|x| *x).collect(),
        }
    }

    /// Returns the device's platform.
    pub fn get_platform(&self) -> Option<Platform> {
        self.platform
    }

    /// Returns true if the device definition defines a pin.
    pub fn has_pin(&self, pin_no: u8) -> bool {
        self.io.contains_key(&pin_no)
    }

    /// Returns Ok(()) if the device definition defines all the given pins.
    pub fn has_pins<T>(&self, pins: T) -> Result<()> where
        T: IntoIterator<Item = u8>
    {
        for pin_no in pins {
            if !self.has_pin(pin_no) {
                return Err(Error::UndefinedPin(pin_no));
            }
        }

        Ok(())
    }

    /// Returns the direction of the pin.
    ///
    /// Returns an error if the pin is not defined.
    pub fn direction_of(&self, pin: u8) -> Result<Direction> {
        self.io.get(&pin)
            .map(|&(dir, _sig)| dir )
            .ok_or(Error::UndefinedPin(pin))
    }

    /// Returns the signal of the pin.
    ///
    /// Returns an error if the pin is not defined.
    #[allow(dead_code)]
    pub fn signal_of(&self, pin: u8) -> Result<SignalClass> {
        self.io.get(&pin)
            .map(|&(_dir, sig)| sig)
            .ok_or(Error::UndefinedPin(pin))
    }
}
