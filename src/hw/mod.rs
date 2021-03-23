//! Drivers for additional hardware for the testbed.

use crate::facility::EnergyMeter;

pub mod acs723;
pub mod pcf8591;

pub use acs723::ACS723;
pub use pcf8591::PCF8591;

pub type PCF8591_ACS723 = (PCF8591, ACS723);

impl EnergyMeter for PCF8591_ACS723 {
    fn current_draw(&self) -> u32 {
        0
    }
}
