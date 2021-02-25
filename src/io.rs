use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::convert::From;
use std::fmt;
use std::fmt::Display;
use std::sync::Mutex;

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

#[derive(Debug)]
pub struct Mapping {
    device: Device,
    numbering: HashMap<u8, u8>,
    inputs: Mutex<Pins<OutputPin>>,
    outputs: Mutex<Pins<InputPin>>,
}

impl Mapping {
    pub fn new<'a, T>(device: &Device, host_target_map: T) -> Result<Mapping> where
        T: IntoIterator<Item = &'a (u8, u8)> {
        let gpio = Gpio::new()?;

        let numbering: HashMap<u8, u8> = host_target_map
            .into_iter()
            .map(|(h_pin, t_pin)| (*h_pin, *t_pin))
            .collect();

        let (mut input_pins, mut output_pins) = (Vec::new(), Vec::new());
        let mut acquired_gpio = numbering
            .iter()
            .map(|(h_pin, t_pin)| { (*t_pin, gpio.get(*h_pin)) });
        for (t_pin_no, acq_res) in acquired_gpio {
            let pin = acq_res?;
            match device.direction_of(t_pin_no)? {
                device::IODirection::In => input_pins.push((t_pin_no, pin.into_output())),
                device::IODirection::Out => output_pins.push((t_pin_no, pin.into_input())),
            }
        }

        Ok(Mapping {
            device: device.clone(),
            numbering,
            inputs: Mutex::new(Pins::new(input_pins.into_iter())),
            outputs: Mutex::new(Pins::new(output_pins.into_iter())),
        })
    }

    pub fn get_inputs(&self) -> &Mutex<Pins<OutputPin>> {
        &self.inputs
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
    pins: HashMap<u8, RefCell<T>>,
}

impl<T> Pins<T> {
    fn new<U>(pins: U) -> Pins<T> where
        U: IntoIterator<Item = (u8, T)>
    {
        Pins {
            pins: pins.into_iter()
                .map(|(pin_no, pin)| (pin_no, RefCell::new(pin)))
                .collect(),
        }
    }

    pub fn has_pin(&self, pin_no: u8) -> bool {
        self.pins.contains_key(&pin_no)
    }

    pub fn get_pin(&self, pin_no: u8) -> Result<RefMut<'_, T>> {
        self.pins.get(&pin_no)
            .ok_or(Error::UndefinedPin(pin_no))
            .and_then(|pin| pin.try_borrow_mut().map_err(|_e| Error::InUse(pin_no)))
    }
}
