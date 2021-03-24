//! Drivers for additional hardware for the testbed.

use crate::facility::EnergyMeter;

pub mod acs723;
pub mod pcf8591;
pub mod hal;

pub use acs723::ACS723;
pub use pcf8591::PCF8591;

use hal::ADCChannel;

impl<'a> EnergyMeter for (ADCChannel<'a>, ACS723) {
    fn current_draw(&self) -> u32 {
        0
    }
}
