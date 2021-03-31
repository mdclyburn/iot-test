//! Drivers for additional hardware for the testbed.

use crate::facility::EnergyMetering;

pub mod ina219;

pub use ina219::INA219;
