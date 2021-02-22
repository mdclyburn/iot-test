use std::fmt;
use std::fmt::Display;

use crate::device::Device;
use crate::io::{IOPin, Mapping};
use crate::testing::{Test, Evaluation, Status};

#[derive(Debug)]
pub struct Testbed<'a> {
    dut: &'a Device,
    pin_mapping: &'a Mapping,
}

impl<'a> Testbed<'a> {
    pub fn new(device: &'a Device, pin_mapping: &'a Mapping) -> Testbed<'a> {
        Testbed {
            dut: device,
            pin_mapping: pin_mapping,
        }
    }

    pub fn execute<T, U>(&self, tests: T, out: &mut U) where
        T: IntoIterator<Item = &'a Test>,
        U: Extend<Evaluation> {
        let mut test_results = Vec::new();
        for test in tests {
            test_results.push(Evaluation::new("bah", Status::Invalid));
        }

        {
            let mut io_pin = self.pin_mapping.get_pin(2).unwrap();
            if let IOPin::Input(ref mut opin) = *io_pin {
                opin.set_high();
                std::thread::sleep(std::time::Duration::from_millis(500));
                opin.set_low();
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }

        {
            let mut io_pin = self.pin_mapping.get_pin(2).unwrap();
            if let IOPin::Input(ref mut opin) = *io_pin {
                opin.set_high();
                std::thread::sleep(std::time::Duration::from_millis(500));
                opin.set_low();
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }

        out.extend(test_results);
    }
}

impl<'a> Display for Testbed<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Testbed\n{}", self.pin_mapping)
    }
}
