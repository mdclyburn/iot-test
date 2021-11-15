/*! Interacting with inputs and outputs.

This module contains types for organizing and managing the I/O between the Raspberry Pi and the device under test.
 */

use std::collections::HashMap;
use std::convert::From;
use std::fmt;
use std::fmt::Display;
use std::iter::{Iterator, IntoIterator};
use std::rc::Rc;

use rppal::gpio;
use rppal::gpio::{Gpio, InputPin, OutputPin};
use rppal::i2c;
use rppal::i2c::I2c;
use rppal::uart;
use rppal::uart::{
    Uart,
    Parity as UARTParity,
};

use crate::comm::{
    Class as SignalClass,
    Direction,
};

/// Testbed I/O result type.
pub type Result<T> = std::result::Result<T, Error>;

/// Set of pins that provide input _to_ the device under test.
pub type DeviceInputs = Pins<OutputPin>;
/// Set of pins that accept output _from_ the device under test.
pub type DeviceOutputs = Pins<InputPin>;

/// Errors related to acquiring and configuring I/O.
#[derive(Debug)]
pub enum Error {
    /// GPIO-specific error.
    Gpio(gpio::Error),
    /// Mapping does not allow I2C.
    I2CUnavailable,
    /// I2C initialization error.
    I2C(i2c::Error),
    /// Reset functionality not defined.
    NoReset,
    /// Mapping does not allow UART.
    UARTUnavailable,
    /// UART initialization error.
    UART(uart::Error),
    /// A provided pin was not defined.
    UndefinedPin(u8),
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Gpio(ref gpio_error) => Some(gpio_error),
            _ => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Error::*;
        match self {
            Gpio(ref e) => write!(f, "error with GPIO interface: {}", e),
            I2CUnavailable => write!(f, "I2C pins (2, 3) are mapped"),
            I2C(ref e) => write!(f, "could not obtain I2C interface: {}", e),
            NoReset => write!(f, "reset functionality is not defined for the device"),
            UARTUnavailable => write!(f, "UART pins (14, 15) are mapped"),
            UART(ref e) => write!(f, "could not obtain UART interface: {}", e),
            UndefinedPin(pin_no) => write!(f, "undefined pin ({}) used", pin_no),
        }
    }
}

impl From<gpio::Error> for Error {
    fn from(e: gpio::Error) -> Self {
        Error::Gpio(e)
    }
}

impl From<i2c::Error> for Error {
    fn from(e: i2c::Error) -> Self {
        Error::I2C(e)
    }
}

impl From<uart::Error> for Error {
    fn from(e: uart::Error) -> Self {
        Error::UART(e)
    }
}

/// Wrapper around a set of pins.
#[derive(Debug)]
pub struct Pins<T> {
    pins: HashMap<u8, T>,
}

impl<T> Pins<T> {
    /// Create a new collection of pins.
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
```ignore
for p in &mut pins {
    println!("Pin #{:02}", p.pin());
    let _res = p.set_high();
    std::thread::sleep(500);
    let _res = p.set_low();
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

/// Properties of a device under test.
pub struct Device {
    io: HashMap<u8, (Direction, SignalClass)>,
    hold_reset: Option<Rc<dyn Fn(&mut DeviceInputs) -> Result<()>>>,
    release_reset: Option<Rc<dyn Fn(&mut DeviceInputs) -> Result<()>>>,
}

impl Device {
    /** Define a new device.

    A device under test has a defined set of inputs and outputs.
    Each I/O has a signal type that it emits or accepts.
    */
    pub fn new<'b, T>(pin_map: T) -> Device where
        T: IntoIterator<Item = &'b (u8, (Direction, SignalClass))> {
        Device {
            io: pin_map.into_iter().map(|x| *x).collect(),
            hold_reset: None,
            release_reset: None,
        }
    }

    /// Define reset functionality for the device.
    pub fn with_reset(self,
                      hold_reset: Rc<dyn Fn(&mut DeviceInputs) -> Result<()>>,
                      release_reset: Rc<dyn Fn(&mut DeviceInputs) -> Result<()>>)
                      -> Self
    {
        let (hold_reset, release_reset) = (Some(hold_reset),
                                           Some(release_reset));

        Self {
            hold_reset,
            release_reset,
            ..self
        }
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

    /// Place the device in reset.
    pub fn hold_in_reset(&self, inputs: &mut DeviceInputs) -> Result<()> {
        let hold_reset = &*self.hold_reset.as_ref().ok_or(Error::NoReset)?;
        hold_reset(inputs)
    }

    /// Release the device from reset.
    pub fn release_from_reset(&self, inputs: &mut DeviceInputs) -> Result<()> {
        let release_reset = &*self.release_reset.as_ref().ok_or(Error::NoReset)?;
        release_reset(inputs)
    }
}

impl fmt::Debug for Device {
    fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }
}

/// Defined UART interfaces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UART {
    /// Full UART built into the Raspberry Pi.
    PL011,
    /// Other UART connected to the Raspberry Pi.
    Custom(String),
}

impl UART {
    /// Path to the UART the variant refers to.
    pub fn path(&self) -> &str {
        use UART::*;
        match self {
            PL011 => "/dev/ttyAMA0",
            Custom(ref path) => path.as_ref(),
        }
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
    reset_pin: Option<u8>,
}

impl Mapping {
    /** Create a new `Mapping`.

    Returns and Ok(Mapping) or an error with the reason for the failure.

    # Examples
    ```ignore
    let mapping = Mapping::new(&device, &[(17, 23), (2, 13)]);
    ```
     */
    pub fn new<'b, T>(device: Device,
                      host_target_map: T,
                      reset_pin: Option<u8>) -> Result<Mapping>
    where
        T: IntoIterator<Item = &'b (u8, u8)>,
    {
        let numbering: HashMap<u8, u8> = host_target_map.into_iter()
            .map(|(h_pin, t_pin)| (*h_pin, *t_pin))
            .collect();
        let reset_pin_v = if let Some(pin) = reset_pin { vec![pin] } else { Vec::new() };

        let used_device_pins = numbering.iter()
            .map(|(_h, t)| *t)
            .chain(reset_pin_v.into_iter());
        device.has_pins(used_device_pins)?;

        Ok(Mapping {
            device,
            numbering,
            reset_pin,
        })
    }

    /// Returns the device definition.
    pub fn get_device(&self) -> &Device {
        &self.device
    }

    /// Returns the host-target pin mapping.
    pub fn get_mapping(&self) -> &HashMap<u8, u8> {
        &self.numbering
    }

    /// Returns the reset pin number.
    pub fn get_reset_pin(&self) -> Option<u8> {
        self.reset_pin
    }

    /** Returns GPIO pins that are inputs _to the device_ (i.e., outputs from the testbed).

    This function returns Ok([`DeviceInputs`]) only when *all* pins are available.
    The pins defined in the mapping must not be held elsewhere in the program.
     */
    pub fn get_gpio_inputs(&self) -> Result<DeviceInputs> {
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
    pub fn get_gpio_outputs(&self) -> Result<DeviceOutputs> {
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

    /** Configures and returns the I2C interface.

    # Errors
    - If the I/O mapping has mapped the pins used for the I2C bus, this function returns `Error::I2CUnavailable`.
    - If the underlying implementation encounters an error initializing I2C, this function returns `Error::I2C`.
     */
    pub fn get_i2c(&self) -> Result<I2c> {
        let i2c_pins_mapped =
            self.numbering.contains_key(&2)
            || self.numbering.contains_key(&3);
        if i2c_pins_mapped {
            Err(Error::I2CUnavailable)
        } else {
            Ok(I2c::new()?)
        }
    }

    /// Retrieves the UART interface.
    ///
    /// If using the UART built into the Raspberry Pi, `which_uart` must be `UART::PL011` to do pin mapping checking.
    pub fn get_uart(&self, which_uart: &UART) -> Result<Uart>
    {
        // Must check the pins that this UART uses.
        if *which_uart == UART::PL011
            && (self.numbering.contains_key(&14) || self.numbering.contains_key(&15))
        {
            Err(Error::UARTUnavailable)
        } else {
            // Use hard-coded values here to avoid complexity
            // in code wanting to use the UART.
            println!("Opening UART: {}", which_uart.path());
            let mut uart = Uart::with_path(which_uart.path(), 115_200, UARTParity::Even, 8, 1)?;
            uart.set_hardware_flow_control(false)?;
            Ok(uart)
        }
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
