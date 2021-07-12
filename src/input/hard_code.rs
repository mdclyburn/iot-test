use std::collections::HashMap;
use std::path::Path;

use crate::comm::{Direction, Class as SignalClass, Signal};
use crate::device::Device;
use crate::facility::EnergyMetering;
use crate::hw::INA219;
use crate::io::Mapping;
use crate::sw::{PlatformSupport, Platform};
use crate::sw::platform::Tock;
use crate::testing::testbed::Testbed;
use crate::testing::criteria::{
    Criterion,
    GPIOCriterion,
    EnergyCriterion,
    EnergyStat,
    Timing,
    TraceCondition,
    TraceCriterion,
};
use crate::testing::test::{
    Operation,
    Test,
};

use super::{Result,
            TestbedConfigReader};
use super::error::Error;

#[derive(Debug)]
pub struct HardCodedTestbed {  }

impl HardCodedTestbed {
    pub fn new() -> HardCodedTestbed {
        HardCodedTestbed {  }
    }
}

impl TestbedConfigReader for HardCodedTestbed {
    fn create(&self) -> Result<Testbed> {
        // physical mapping
        let device = Device::new(
            &[
                // (13, (Direction::Out, SignalClass::Digital)), // D0
                // (14, (Direction::Out, SignalClass::Digital)), // D1
                // (19, (Direction::Out, SignalClass::Digital)), // D6
                // (20, (Direction::Out, SignalClass::Digital)), // D7
                (23, (Direction::In, SignalClass::Digital)),  // reset
            ]);

        let mapping = Mapping::new(
            &device,
            // Host to device-under-test pin mapping.
            &[(17, 23), // Reset

              // (14, 13), // Parallel tracing pin 0
              // (15, 14), // Parallel tracing pin 1
              // (18, 19), // Parallel tracing pin 2
              // (23, 20), // Parallel tracing pin 3
            ],
            // Parallel tracing pins (by device pin number).
            &[],
        ).unwrap();

        // Energy metering
        let ina219: Box<dyn EnergyMetering> = Box::new(
            INA219::new(mapping.get_i2c().unwrap(), 0x40).unwrap());
        let energy_meters: HashMap<String, Box<dyn EnergyMetering>> = (vec![
            ("system".to_string(), ina219)
        ]).into_iter()
            .collect();

        // platform support
        let platform = Tock::new(
            "hail",
            Path::new("/usr/local/bin/tockloader"),
            Path::new("/home/ubuntu/work/tock"),
            Path::new("/home/ubuntu/work/apps/tock"));

        let testbed = Testbed::new(
            mapping,
            Box::new(platform),
            energy_meters);

        Ok(testbed)
    }
}