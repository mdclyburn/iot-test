//! Support for the INA219 sensor.

use std::cell::{RefCell, RefMut};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use rppal::i2c::I2c;

use crate::facility::EnergyMetering;

/// INA219 register addresses.
#[allow(unused)]
mod register {
    pub const CONFIGURATION: u8 = 0x00;
    pub const SHUNT_VOLTAGE: u8 = 0x01;
    pub const BUS_VOLTAGE: u8   = 0x02;
    pub const POWER: u8         = 0x03;
    pub const CURRENT: u8       = 0x04;
    pub const CALIBRATION: u8   = 0x05;
}

/// Conversion factor when reading bus voltage (4mV per value).
const BUS_VOLTAGE_LSB: f32 = 0.004;

/// Driver for the TI INA219 current sensor.
#[derive(Debug)]
pub struct INA219 {
    address: u8,
    i2c: Mutex<RefCell<I2c>>,
    current_lsb: f32,
}

impl INA219 {
    const MAX_CURRENT_AMPS: f32 = 0.8;
    const SHUNT_RESISTOR_OHMS: f32 = 0.1;

    /// Create a new INA219 driver.
    pub fn new(i2c: I2c, address: u8) -> Result<INA219, String> {
        let ina = INA219 {
            address,
            i2c: Mutex::new(RefCell::new(i2c)),
            current_lsb: Self::MAX_CURRENT_AMPS / 2f32.powi(15),
        };
        ina.init()?;
        println!("Current LSB: {}", ina.current_lsb);

        Ok(ina)
    }

    /// Reset the INA219.
    pub fn reset(&self) -> Result<(), String> {
        // Just write the default configuration, as that should be safe.
        let config = 0x399F | ((1 as u16) << 15);
        self.write(register::CONFIGURATION, config)?;
        thread::sleep(Duration::from_micros(40)); // need >=40us after reset.

        Ok(())
    }

    /// Return the current current draw in milliamps.
    pub fn current(&self) -> Result<f32, String> {
        Ok(self.read(register::CURRENT)? as f32 * self.current_lsb * 1_000f32)
    }

    /// Return the current power measurement in milliwatts.
    pub fn power(&self) -> Result<f32, String> {
        Ok(self.read(register::POWER)? as f32 * 20.0f32 * self.current_lsb * 1_000f32)
    }

    /// Return the bus voltage in volts.
    pub fn bus_voltage(&self) -> Result<f32, String> {
        let raw = self.read(register::BUS_VOLTAGE)?;
        Ok(((raw >> 3) as f32) * BUS_VOLTAGE_LSB)
    }

    fn init(&self) -> Result<(), String> {
        self.with_i2c(|mut i2c| {
            i2c.set_slave_address(self.address as u16)
                .map_err(|e| format!("failed to set peripheral address: {}", e))
        })?;
        self.reset()?;

        /* Set configuration; see INA219 documentation for details.

        - gain amplifier: /4 (+/- 160mV)
        - ADC resolution/averaging: 12-bit
        - bus ADC resolution: 12-bit, 532 us conversion time
        - shunt ADC resolution: 12-bit, 532 us conversion time
        - operating mode: shunt + bus, continuous
        -----
        Should yield a +/- 1.6A range with 0.390625mA resolution.
         */
        let config = 0b0_0_0_1_10_0011_0011_111;
        self.write(register::CONFIGURATION, config)?;

        let cal = (0.04096f32 / (self.current_lsb * Self::SHUNT_RESISTOR_OHMS)) as u16;
        self.write(register::CALIBRATION, cal)?;
        println!("Calibration: {}", cal);

        let current_current = self.current()?;
        println!("Current draw: {}", current_current);

        Ok(())
    }

    fn read(&self, reg_addr: u8) -> Result<u16, String> {
        let mut out = [0xff; 2];
        self.with_i2c(|i2c| {
            i2c.write_read(&[reg_addr], &mut out)
                .map_err(|e| format!("failed to perform write-read: {}", e))?;
            Ok(((out[0] as u16) << 8) | (out[1] as u16))
        })
    }

    fn write(&self, reg_addr: u8, value: u16) -> Result<(), String> {
        let buf = [
            reg_addr,
            (value >> 8) as u8,
            (value & 0xFF) as u8,
        ];
        self.with_i2c(|mut i2c| {
            i2c.write(&buf)
                .map(|_bytes_written| ())
                .map_err(|e| format!("failed to write {:X} register: {}", reg_addr, e))
        })
    }

    fn with_i2c<F, T>(&self, op: F) -> Result<T, String>
    where
        F: FnOnce(RefMut<'_, I2c>) -> Result<T, String>
    {
        let i2c_cell = self.i2c.lock()
            .map_err(|e| format!("failed to lock I2C interface: {}", e))?;

        op(i2c_cell.borrow_mut())
    }
}

impl EnergyMetering for INA219 {
    fn current(&self) -> f32 {
        self.current().unwrap()
    }

    fn power(&self) -> f32 {
        self.power().unwrap()
    }

    fn cooldown_duration(&self) -> Duration {
        Duration::from_micros(532)
    }
}
