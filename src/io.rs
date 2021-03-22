/*! Interacting with inputs and outputs.

This module contains types for organizing and managing the I/O between the Raspberry Pi and the device under test.
 */

use std::collections::HashMap;
use std::convert::From;
use std::fmt;
use std::fmt::Display;
use std::iter::{Iterator, IntoIterator};

use rppal::gpio;
use rppal::gpio::{Gpio, InputPin, OutputPin};
use rppal::i2c::I2c;

use crate::comm::Direction;
use crate::device;
use crate::device::Device;
use crate::energy::EnergyMeter;

type Result<T> = std::result::Result<T, Error>;

/// Set of pins that provide input _to_ the device under test.
pub type DeviceInputs = Pins<OutputPin>;
/// Set of pins that accept output _from_ the device under test.
pub type DeviceOutputs = Pins<InputPin>;

/// Errors related to acquiring and configuring I/O.
#[derive(Debug)]
pub enum Error {
    /// Device-specific error
    Device(device::Error),
    /// GPIO-specific error
    Gpio(gpio::Error),
    /// Requested pin is not mapped
    UndefinedPin(u8),
    /// Impossible mapping specified
    InvalidMap(String),
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
            Error::Device(ref e) => write!(f, "error with target interface: {}", e),
            Error::Gpio(ref e) => write!(f, "error with GPIO interface: {}", e),
            Error::UndefinedPin(pin_no) => write!(f, "target pin {} not mapped", pin_no),
            Error::InvalidMap(ref msg) => write!(f, "bad mapping: {}", msg),
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

/** Interface to I/O between the testbed and the device under test.

`Mapping` defines the interface between the testbed and the device under test.
Creating a mapping with [`Mapping::new`] ensures a valid testbed-device configuration provided the [`Device`] definition is correct.
*/
#[derive(Debug)]
pub struct Mapping {
    device: Device,
    numbering: HashMap<u8, u8>,
    energy_meter: Option<I2c>,
}

impl Mapping {
    /** Create a new `Mapping`.

    Returns and Ok(Mapping) or an error with the reason for the failure.

    # Examples
    ```
    let mapping = Mapping::new(&device, &[(17, 23), (2, 13)]);
    ```
     */
    pub fn new<'a, T>(device: &Device,
                      host_target_map: T,
                      provide_i2c: bool) -> Result<Mapping>
    where
        T: IntoIterator<Item = &'a (u8, u8)>
    {
        let numbering: HashMap<u8, u8> = host_target_map
            .into_iter()
            .map(|(h_pin, t_pin)| (*h_pin, *t_pin))
            .collect();

        device.has_pins(numbering.iter().map(|(_h, t)| *t))?;

        // Allocate I2C pins.
        if provide_i2c {
            let mapped = numbering.iter()
                .find(|(h, _t)| **h == 3 || **h == 5);
            if let Some((h_pin, _t_pin)) = mapped {
                let msg = format!("Pin {} cannot be allocated for separate I2C because it is mapped.", h_pin);
                return Err(Error::InvalidMap(msg));
            }
        }

        Ok(Mapping {
            device: device.clone(),
            numbering,
            energy_meter: None,
        })
    }

    /** Returns GPIO pins that are inputs _to the device_ (i.e., outputs from the testbed).

    This function returns Ok([`DeviceInputs`]) only when *all* pins are available.
    The pins defined in the mapping must not be held elsewhere in the program.
     */
    pub fn get_inputs(&self) -> Result<DeviceInputs> {
        let input_numbering = self.numbering.iter()
            .map(|(h, t)| (*h, *t))
            .filter(|(_h, t)| self.device.direction_of(*t).unwrap() == Direction::In);
        let mut inputs = Vec::new();
        let gpio = Gpio::new()?;

        for (h_pin, t_pin) in input_numbering {
            let pin = gpio.get(h_pin)?;
            inputs.push((t_pin, pin.into_output()));
        }

        Ok(DeviceInputs::new(inputs))
    }

    /** Returns GPIO pins that are outputs _from the device_ (i.e., inputs to the testbed).

    This function returns Ok([`DeviceOutputs`]) only when *all* pins are available.
    The pins defined in the mapping must not be held elsewhere in the program.
     */
    pub fn get_outputs(&self) -> Result<DeviceOutputs> {
        let output_numbering = self.numbering.iter()
            .map(|(h, t)| (*h, *t))
            .filter(|(_h, t)| self.device.direction_of(*t).unwrap() == Direction::Out);
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
        write!(f, "{:^9} {:^3} {:^7}\n", "testbed", "dir", "device")?;
        for (h_pin, t_pin) in &self.numbering {
            let dev_io_dir = self.device.direction_of(*t_pin).unwrap();
            let dir_str = if dev_io_dir == Direction::In {
                "->"
            } else {
                "<-"
            };
            write!(f, "   P{:02}    {}   P{:02}\n", h_pin, dir_str, t_pin)?;
        }

        Ok(())
    }
}

/// Wrapper around a set of pins.
#[derive(Debug)]
pub struct Pins<T> {
    pins: HashMap<u8, T>,
}

impl<T> Pins<T> {
    /// Create a new collectio of pins.
    fn new<U>(pins: U) -> Pins<T> where
        U: IntoIterator<Item = (u8, T)>
    {
        Pins {
            pins: pins.into_iter()
                .map(|(pin_no, pin)| (pin_no, pin))
                .collect(),
        }
    }

    /// Returns a reference to the specified pin.
    #[allow(dead_code)]
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
            .map(|(_pin_no, pin)| pin)
            .collect();
        Ok(pins)
    }
}

/** An iterator over mutable pins.

This iterator allows the pins that are iterated over to change state
(e.g., set/clear interrupts or change logic state).

# Examples
```
for p in &mut pins {
    println!("Pin #{:02}", p.pin());
    p.set_high()?;
    thread::sleep(500);
    p.set_low()?;
}
```
*/
pub struct PinsIterMut<'a, T> {
    pins_it: std::collections::hash_map::IterMut<'a, u8, T>,
}

impl<'a, T> PinsIterMut<'a, T> {
    fn new(pins: &'a mut HashMap<u8, T>) -> PinsIterMut<'a, T> {
        PinsIterMut {
            pins_it: pins.iter_mut(),
        }
    }
}

impl<'a, T> Iterator for PinsIterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        self.pins_it.next()
            .map(|(_pin_no, pin)| pin)
    }
}

impl<'a, T> IntoIterator for &'a mut Pins<T> {
    type Item = &'a mut T;
    type IntoIter = PinsIterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        PinsIterMut::new(&mut self.pins)
    }
}
