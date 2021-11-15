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
    ParallelTraceCondition,
    ParallelTraceCriterion,
    SerialTraceCondition,
    SerialTraceCriterion,
};
use clockwise_common::facility::EnergyMetering;
use clockwise_common::hw::INA219;
use clockwise_common::input::TestProvider;
use clockwise_common::io;
use clockwise_common::io::{
    Device,
    DeviceInputs,
    Mapping,
    UART,
};
use clockwise_common::test::{Operation, Test};

use crate::sw::platform::Tock;
use crate::testing::testbed::Testbed;

use super::{Result,
            TestbedConfigReader};

/// Testbed created from code compiled into the binary.
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
        let host_to_device_pins = [
            // (13, (Direction::Out, SignalClass::Digital)), // D0
            // (14, (Direction::Out, SignalClass::Digital)), // D1
            // (19, (Direction::Out, SignalClass::Digital)), // D6
            // (20, (Direction::Out, SignalClass::Digital)), // D7
            (23, (Direction::In, SignalClass::Digital)),  // reset
        ];

        // reset functions
        let hold_reset_fn: Rc<dyn Fn(&mut DeviceInputs) -> io::Result<()>> = Rc::new(
            |to_device| {
                let reset_pin = to_device.get_pin_mut(23)?;
                reset_pin.set_low();
                thread::sleep(Duration::from_millis(10));

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

              // (14, 13), // Parallel tracing pin 0
              // (15, 14), // Parallel tracing pin 1
              // (18, 19), // Parallel tracing pin 2
              // (23, 20), // Parallel tracing pin 3
            ],
            // Parallel tracing pins (by device pin number).
            &[],
            Some(23),
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
            energy_meters,
            None,
            Some(UART::PL011));

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
                // Test::new(
                //     "radio-packet-tx",
                //     (&["radio_send_app"]).into_iter().map(|x| *x),
                //     (&[]).into_iter().copied(),
                //     &[Operation::reset_device(),
                //       Operation::idle_testbed(Duration::from_millis(5000))],
                //     &[Criterion::Energy(EnergyCriterion::new("system-total", EnergyStat::Total)
                //                         .with_max(350.0))]),

                // Test::new(
                //     "no-app-test",
                //     (&[]).into_iter().map(|x| *x),
                //     &[Operation { time: 0, pin_no: 23, input: Signal::Digital(false) },
                //       Operation { time: 200, pin_no: 23, input: Signal::Digital(false) }],
                //     &[Criterion::Energy(EnergyCriterion::new("system", EnergyStat::Average)
                //                         .with_min(10.0))]),

                // Test::new(
                //     "blink-trace",
                //     (&[]).into_iter().copied(),
                //     (&["capsule/led/command/on", "capsule/led/command/off"]).into_iter().copied(),
                //     &[Operation { time: 0, pin_no: 23, input: Signal::Digital(false) },
                //       Operation { time: 3000, pin_no: 23, input: Signal::Digital(true) }],
                //     &[Criterion::ParallelTrace(ParallelTraceCriterion::new(&[ParallelTraceCondition::new(2).with_extra_data(1),
                //                                                              ParallelTraceCondition::new(1).with_timing(Timing::Relative(Duration::from_millis(50)),
                //                                                                                                         Duration::from_millis(5))
                //                                                              .with_extra_data(1)]))]),

                Test::new(
                    "serial-blink-trace",
                    (&[]).into_iter().copied(),
                    (&[]).into_iter().copied(),
                    &[Operation::at(0).idle_sync(Duration::from_millis(3000))],
                    &[Criterion::SerialTrace(
                        SerialTraceCriterion::new(&[
                            SerialTraceCondition::new(&[0x6c, 0x65, 0x64, 0x20, 0x6f, 0x6e]),
                            SerialTraceCondition::new(&[0x6c, 0x65, 0x64, 0x20, 0x6f, 0x6e])
                                .with_timing(Timing::Relative(Duration::from_millis(250)),
                                             Duration::from_millis(25)),
                            SerialTraceCondition::new(&[0x6c, 0x65, 0x64, 0x20, 0x6f, 0x6e])
                                .with_timing(Timing::Relative(Duration::from_millis(0)),
                                             Duration::from_millis(10))]))],
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
