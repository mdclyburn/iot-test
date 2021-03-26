use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::sync::{Mutex, MutexGuard};

use rppal::i2c::I2c;

use crate::facility::EnergyMetering;

use super::hal::{ADC, ADCChannel};

#[allow(unused)]
mod register {
    pub const CONFIGURATION: u8 = 0x00;
    pub const SHUNT_VOLTAGE: u8 = 0x01;
    pub const BUS_VOLTAGE: u8   = 0x02;
    pub const POWER: u8         = 0x03;
    pub const CURRENT: u8       = 0x04;
    pub const CALIBRATION: u8   = 0x05;
}

/// Driver for the TI INA219 current sensor.
#[derive(Debug)]
pub struct INA219 {
    address: u8,
    i2c: Mutex<RefCell<I2c>>,
}

impl INA219 {
    pub fn new(i2c: I2c, address: u8) -> Result<INA219, String> {
        let ina = INA219 {
            address,
            i2c: Mutex::new(RefCell::new(i2c)),
        };
        ina.init()?;

        Ok(ina)
    }

    pub fn read_current(&self) -> Result<u16, String> {
        self.read(register::CURRENT)
    }

    fn init(&self) -> Result<(), String> {
        let i2c = self.lock_i2c()?;
        let result = (*i2c).borrow_mut()
            .set_slave_address(self.address as u16)
            .map_err(|e| format!("failed to set peripheral address: {}", e));

        result
    }

    fn read(&self, reg_addr: u8) -> Result<u16, String> {
        let mut out = [0xff; 2];
        let i2c = self.lock_i2c()?;

        (*i2c).borrow_mut().write_read(&[reg_addr], &mut out)
            .map_err(|e| format!("failed to perform write-read: {}", e))?;

        Ok(((out[0] as u16) << 8) | (out[1] as u16))
    }

    fn write(&self, reg_addr: u8, value: u16) -> Result<(), String> {
        let buf = [reg_addr,
                   (value >> 8) as u8,
                   (value & 0xFF) as u8];
        let i2c = self.lock_i2c()?;

        let result = (*i2c).borrow_mut().write(&buf)
            .map(|_bytes_written| ())
            .map_err(|e| format!("failed to write register: {}", e));

        result
    }

    fn lock_i2c(&self) -> Result<MutexGuard<'_, RefCell<I2c>>, String> {
        self.i2c.lock()
            .map_err(|e| format!("failed to lock I2C interface: {}", e))
    }
}

impl EnergyMetering for INA219 {
    fn current_draw(&self) -> u32 {
        self.read_current().unwrap() as u32
    }
}
