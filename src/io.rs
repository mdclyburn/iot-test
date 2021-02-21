use std::cell::RefCell;
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
// To change an input, an output pin needs to be manipulated.
// To monitor an output, an input pin needs to be queried.
#[derive(Debug)]
pub enum IOPin {
    Input(RefCell<OutputPin>),
    Output(RefCell<InputPin>),
}

#[derive(Debug)]
pub struct Mapping {
    target_io: HashMap<u8, IOPin>,
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
                        t_pin, IOPin::Input(RefCell::new(acq_res?.into_output()))),
                device::IODirection::Out =>
                    mapping.target_io.insert(
                        t_pin, IOPin::Output(RefCell::new(acq_res?.into_input()))),
            };
        }

        Ok(mapping)
    }

    pub fn get_pin(&self, pin_no: u8) -> Result<&IOPin> {
        self.target_io.get(&pin_no)
            .ok_or(Error::UndefinedPin(pin_no))
    }
}

impl Display for Mapping {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "I/O mapping:\n")?;
        for (target_pin, io_pin) in &self.target_io {
            match io_pin {
                IOPin::Input(ref cell) =>
                    write!(f, "P{:02} ---> P{:02}", cell.borrow_mut().pin(), target_pin)?,
                IOPin::Output(ref cell) =>
                    write!(f, "P{:02} <--- P{:02}", cell.borrow_mut().pin(), target_pin)?,
            };
        }

        Ok(())
    }
}
