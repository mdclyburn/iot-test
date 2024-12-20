//! Hard-coded Testbed and Test providers.

use std::collections::HashMap;
use std::path::Path;
use std::thread;
use std::time::Duration;
use std::rc::Rc;

use clockwise_common::comm::{Direction, Class as SignalClass};
#[allow(unused_imports)]
use clockwise_common::criteria::{
    Criterion,
    GPIOCriterion,
    EnergyCriterion,
    EnergyStat,
    Timing,
    SerialTraceCondition,
    SerialTraceCriterion,
};
use clockwise_common::facility::EnergyMetering;
use clockwise_common::hw::INA219;
use clockwise_common::input::{TestProvider, TestbedProvider};
use clockwise_common::io;
use clockwise_common::io::{
    Device,
    DeviceInputs,
    Mapping,
    UART,
};
use clockwise_common::sw::platform::Tock;
use clockwise_common::test::{Operation, Test};
use clockwise_common::testbed::Testbed;
use clockwise_common::trace::{
    BenchmarkMetadata,
    TraceKind,
    WaypointMetadata,
};

/// Testbed created from code compiled into the binary.
#[derive(Debug)]
pub struct HardCodedTestbed {  }

impl HardCodedTestbed {
    pub fn new() -> HardCodedTestbed {
        HardCodedTestbed {  }
    }
}

impl TestbedProvider for HardCodedTestbed {
    fn create(&self) -> Result<Testbed, String> {
        // physical mapping
        let host_to_device_pins = [
            (13, (Direction::In, SignalClass::Digital)), // D0
            // (14, (Direction::Out, SignalClass::Digital)), // D1
            // (19, (Direction::Out, SignalClass::Digital)), // D6
            // (20, (Direction::Out, SignalClass::Digital)), // D7
            (23, (Direction::In, SignalClass::Digital)),  // reset
        ];

        // reset functions
        let hold_reset_fn: Rc<dyn Fn(&mut DeviceInputs) -> io::Result<()>> = Rc::new(
            |to_device| {
                let reset_pin = to_device.get_pin_mut(23)?;
                reset_pin.set_reset_on_drop(false);
                reset_pin.set_low();
                thread::sleep(Duration::from_millis(200));

                Ok(())
            });

        let release_reset_fn: Rc<dyn Fn(&mut DeviceInputs) -> io::Result<()>> = Rc::new(
            |to_device| {
                let reset_pin = to_device.get_pin_mut(23)?;
                reset_pin.set_high();

                Ok(())
            });

        let device = Device::new(&host_to_device_pins)
            .with_reset(hold_reset_fn.clone(), release_reset_fn.clone());

        let mapping = Mapping::new(
            device,
            // Host to device-under-test pin mapping.
            &[(17, 23), // Reset
              (20, 13),
            ],
            Some(23),
        ).unwrap();

        // Energy metering
        let ina219: Box<dyn EnergyMetering> = Box::new(
            INA219::new(mapping.get_i2c().unwrap(), 0x40).unwrap());
        let energy_meters: HashMap<String, Box<dyn EnergyMetering>> = (vec![
            ("system".to_string(), ina219)
        ]).into_iter()
            .collect();

        // Tracing capabilities
        // let tracing = {
        //     let benchmark_tracing = TraceKind::Performance(
        //         BenchmarkMetadata::new(
        //             "samples",
        //             &[
        //                 WaypointMetadata { label: "sam4l-adc".to_string() },
        //                 WaypointMetadata { label: "adc-capsule".to_string() },
        //                 WaypointMetadata { label: "application".to_string() },
        //                 WaypointMetadata { label: "dac-capsule".to_string() },
        //             ]));

        //     vec![
        //         (benchmark_tracing, UART::PL011),
        //     ]
        // };
        let tracing = vec![];

        // platform support
        let platform = Tock::new(
            "hail",
            Path::new("/usr/local/bin/tockloader"),
            Path::new("/home/ubuntu/work/tock"),
            Path::new("/home/ubuntu/work/apps/tock"));

        let testbed = Testbed::new(
            mapping,
            Box::new(platform),
            energy_meters,
            None,
            None,
            tracing);

        Ok(testbed)
    }
}

/// Test adapter providing tests built from code compiled into the binary.
#[derive(Debug)]
pub struct HardCodedTests {
    tests: Vec<Test>,
}

impl HardCodedTests {
    pub fn new() -> HardCodedTests {
        HardCodedTests {
            tests: vec![
                Test::new(
                    "empty-test",
                    (&[]).into_iter().copied(),
                    (&[]).into_iter().copied(),
                    &[Operation::at(0).idle_sync(Duration::from_millis(18_000_000))],
                    &[Criterion::Energy(EnergyCriterion::new("system", EnergyStat::Average)),
                      Criterion::Energy(EnergyCriterion::new("system", EnergyStat::Total))],
                    true),
            ],
        }
    }
}

impl TestProvider for HardCodedTests {
    fn tests(&self) -> Box<dyn Iterator<Item = Test> + '_>
    {
        let it = self.tests.iter()
            .cloned();

        Box::new(it)
    }
}
