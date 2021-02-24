use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::convert::From;
use std::fmt;
use std::fmt::Display;

use rppal::gpio;
use rppal::gpio::{Gpio, InputPin, OutputPin};

use crate::device;
use crate::device::Device;

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Device(device::Error),
    Gpio(gpio::Error),
    UndefinedPin(u8),
    InUse(u8),
    WrongDir,
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            Error::Device(ref dev_error) => Some(dev_error),
            Error::Gpio(ref gpio_error) => Some(gpio_error),
            _ => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Device(_) => write!(f, "error with target interface"),
            Error::Gpio(_) => write!(f, "error with GPIO interface"),
            Error::UndefinedPin(pin_no) => write!(f, "target pin {} not mapped", pin_no),
            Error::InUse(pin_no) => write!(f, "target pin {} in use elsewhere", pin_no),
            Error::WrongDir => write!(f, "expected an input pin, got an output pin or vice versa"),
        }
    }
}

impl From<gpio::Error> for Error {
    fn from(e: gpio::Error) -> Self {
        Error::Gpio(e)
    }
}

impl From<device::Error> for Error {
    fn from(e: device::Error) -> Self {
        Error::Device(e)
    }
}

// Matching enums for IO from the testbed perspective.
// To change an input to the DUT, an output pin needs to be manipulated.
// To monitor an output to the DUT, an input pin needs to be queried.
#[derive(Debug)]
pub enum IOPin {
    Input(OutputPin),
    Output(InputPin),
}

impl IOPin {
    pub fn expect_output(&mut self) -> Result<&mut OutputPin> {
        if let IOPin::Input(ref mut p) = self{
            Ok(p)
        } else {
            Err(Error::WrongDir)
        }
    }

    pub fn expect_input(&mut self) -> Result<&mut InputPin> {
        if let IOPin::Output(ref mut p) = self {
            Ok(p)
        } else {
            Err(Error::WrongDir)
        }
    }
}

#[derive(Debug)]
pub struct Mapping {
    target_io: HashMap<u8, RefCell<IOPin>>,
}

impl Mapping {
    pub fn new<'a, T>(device: &Device, host_target_map: T) -> Result<Mapping> where
        T: IntoIterator<Item = &'a (u8, u8)> {
        let mut mapping = Mapping {
            target_io: HashMap::new()
        };
        let gpio = Gpio::new()?;
        let it_acquire_io = host_target_map
            .into_iter()
            .map(|(h_pin, t_pin)| {
                (*h_pin, *t_pin, gpio.get(*h_pin))
            });

        for (h_pin, t_pin, acq_res) in it_acquire_io {
            match device.direction_of(t_pin)? {
                device::IODirection::In =>
                    mapping.target_io.insert(
                        t_pin, RefCell::new(IOPin::Input(acq_res?.into_output()))),
                device::IODirection::Out =>
                    mapping.target_io.insert(
                        t_pin, RefCell::new(IOPin::Output(acq_res?.into_input()))),
            };
        }

        Ok(mapping)
    }

    pub fn get_pin(&self, pin_no: u8) -> Result<RefMut<'_, IOPin>> {
        self.target_io.get(&pin_no)
            .ok_or(Error::UndefinedPin(pin_no))
            .and_then(|pin| pin.try_borrow_mut().map_err(|_e| Error::InUse(pin_no)))
    }
}

impl Display for Mapping {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "I/O mapping:\n")?;
        for (target_pin, io_pin) in &self.target_io {
            match *io_pin.borrow() {
                IOPin::Input(ref output) =>
                    write!(f, "P{:02} ---> P{:02}", output.pin(), target_pin)?,
                IOPin::Output(ref input) =>
                    write!(f, "P{:02} <--- P{:02}", input.pin(), target_pin)?,
            };
        }

        Ok(())
    }
}
