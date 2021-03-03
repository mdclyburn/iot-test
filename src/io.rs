use std::cell::{RefCell, Ref, RefMut};
use std::collections::HashMap;
use std::convert::From;
use std::fmt;
use std::fmt::Display;
use std::sync::{Arc, Mutex};

use rppal::gpio;
use rppal::gpio::{Gpio, InputPin, OutputPin};

use crate::device;
use crate::device::Device;

type Result<T> = std::result::Result<T, Error>;

pub type DeviceInputs = Pins<OutputPin>;
pub type DeviceOutputs = Pins<InputPin>;

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

#[derive(Debug)]
pub struct Mapping {
    device: Device,
    numbering: HashMap<u8, u8>,
}

impl Mapping {
    pub fn new<'a, T>(device: &Device, host_target_map: T) -> Result<Mapping> where
        T: IntoIterator<Item = &'a (u8, u8)> {
        let numbering: HashMap<u8, u8> = host_target_map
            .into_iter()
            .map(|(h_pin, t_pin)| (*h_pin, *t_pin))
            .collect();

        device.has_pins(numbering.iter().map(|(h, t)| *t))?;

        Ok(Mapping {
            device: device.clone(),
            numbering,
        })
    }

    pub fn get_inputs(&self) -> Result<DeviceInputs> {
        let input_numbering = self.numbering.iter()
            .map(|(h, t)| (*h, *t))
            .filter(|(h, t)| self.device.direction_of(*t).unwrap() == device::IODirection::In);
        let mut inputs = Vec::new();
        let gpio = Gpio::new()?;

        for (h_pin, t_pin) in input_numbering {
            let pin = gpio.get(h_pin)?;
            inputs.push((t_pin, pin.into_output()));
        }

        Ok(DeviceInputs::new(inputs))
    }

    pub fn get_outputs(&self) -> Result<DeviceOutputs> {
        let output_numbering = self.numbering.iter()
            .map(|(h, t)| (*h, *t))
            .filter(|(h, t)| self.device.direction_of(*t).unwrap() == device::IODirection::Out);
        let mut outputs = Vec::new();
        let gpio = Gpio::new()?;

        for (h_pin, t_pin) in output_numbering {
            let pin = gpio.get(h_pin)?;
            outputs.push((t_pin, pin.into_input()));
        }

        Ok(DeviceOutputs::new(outputs))
    }
}

impl Display for Mapping {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "I/O mapping:\n")?;
        for (h_pin, t_pin) in &self.numbering {
            let dev_io_dir = self.device.direction_of(*t_pin).unwrap();
            let dir_str = if dev_io_dir == device::IODirection::In {
                "--->"
            } else {
                "<---"
            };
            write!(f, "P{:02} {} P{:02}", h_pin, dir_str, t_pin)?;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct Pins<T> {
    pins: HashMap<u8, T>,
}

impl<T> Pins<T> {
    fn new<U>(pins: U) -> Pins<T> where
        U: IntoIterator<Item = (u8, T)>
    {
        Pins {
            pins: pins.into_iter()
                .map(|(pin_no, pin)| (pin_no, pin))
                .collect(),
        }
    }

    /// Returns whether the device has the pin mapped.
    pub fn has_pin(&self, pin_no: u8) -> bool {
        self.pins.contains_key(&pin_no)
    }

    /// Returns a reference to the specified pin.
    pub fn get_pin(&self, pin_no: u8) -> Result<&T> {
        self.pins.get(&pin_no)
            .ok_or(Error::UndefinedPin(pin_no))
    }

    /// Returns a mutable reference to the specified pin.
    pub fn get_pin_mut(&mut self, pin_no: u8) -> Result<&mut T> {
        self.pins.get_mut(&pin_no)
            .ok_or(Error::UndefinedPin(pin_no))
    }

    /// Returns all configured pins as plain references.
    pub fn get(&self) -> Result<Vec<&T>> {
        let pins = self.pins.iter()
            .map(|(pin_no, pin)| pin)
            .collect();
        Ok(pins)
    }
}
