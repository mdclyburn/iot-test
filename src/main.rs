//! IoT testing tool

use std::path::Path;

mod comm;
mod device;
mod facility;
mod hw;
mod io;
mod sw;
mod testing;

use crate::comm::{Direction, Class as SignalClass, Signal};
use crate::device::Device;
use crate::facility::EnergyMetering;
use crate::hw::INA219;
use crate::io::Mapping;
use crate::sw::application::{Application, ApplicationSet};
use crate::sw::{PlatformSupport, Platform};
use crate::sw::platform::Tock;
use crate::testing::{
    Criterion,
    EnergyCriterion,
    EnergyStat,
    GPIOCriterion,
    Test,
    Testbed,
    Operation,
};

fn main() {
    // physical mapping
    let device = Device::new(
        &[(13, (Direction::Out, SignalClass::Digital)), // D0
          (14, (Direction::Out, SignalClass::Digital)), // D1
          (19, (Direction::Out, SignalClass::Digital)), // D6
          (20, (Direction::Out, SignalClass::Digital)), // D7
          (23, (Direction::In, SignalClass::Digital)),  // reset
        ]);
    let mapping = Mapping::new(
        &device,
        &[(17, 23), // reset

          // GPIO tracing
          (14, 13),
          (15, 14),
          (18, 19),
          (23, 20),
        ]
    ).unwrap();

    // energy metering
    let ina219 = INA219::new(mapping.get_i2c().unwrap(), 0x40)
        .unwrap();
    let energy_meters: Vec<(&str, Box<dyn EnergyMetering>)> = vec![("system", Box::new(ina219))];

    // platform support
    let tock_support = Tock::new(
        Path::new("/usr/local/bin/tockloader"),
        Path::new("/home/ubuntu/work/tock"));
    let platforms: Vec<Box<dyn PlatformSupport>> = vec![
        Box::new(tock_support),
    ];

    // applications
    let app_set = ApplicationSet::new(
        &[Application::new("blink", &[(Platform::Tock, Path::new("/home/ubuntu/work/apps/tock/blink.tab"))])]
    );

    let testbed = Testbed::new(
        mapping,
        Platform::Tock,
        platforms,
        energy_meters,
        Some(app_set)).unwrap();
    print!("{}\n", testbed);

    let tests = [
        // Test::new(
        //     "example-blink-test",
        //     (&["blink"]).into_iter().map(|x| *x),
        //     &[Operation { time: 0, pin_no: 23, input: Signal::Digital(true) },
        //       Operation { time: 500, pin_no: 23, input: Signal::Digital(false) }],
        //     &[Criterion::GPIO(GPIOCriterion::Any(13)),
        //       Criterion::Energy(EnergyCriterion::new("system", EnergyStat::Total))
        //     ]),

        // Test::new(
        //     "no-app-test",
        //     (&[]).into_iter().map(|x| *x),
        //     &[Operation { time: 0, pin_no: 23, input: Signal::Digital(false) },
        //       Operation { time: 200, pin_no: 23, input: Signal::Digital(false) }],
        //     &[Criterion::Energy(EnergyCriterion::new("system", EnergyStat::Average)
        //                         .with_min(10.0))]),

        Test::new(
            "trace-capture-alpha",
            (&[]).into_iter().map(|x| *x),
            (&[]).into_iter().map(|x| *x),
            &[Operation { time: 0, pin_no: 23, input: Signal::Digital(false) },
              Operation { time: 2000, pin_no: 23, input: Signal::Digital(true) }],
            &[Criterion::GPIO(GPIOCriterion::Any(13)),
              Criterion::GPIO(GPIOCriterion::Any(14)),
              Criterion::GPIO(GPIOCriterion::Any(19)),
              Criterion::GPIO(GPIOCriterion::Any(20))])
    ];

    for test in &tests {
        print!("{}\n\n", test);
    }

    let res = testbed.execute(&tests);
    if let Ok(results) = res {
        for r in results {
            println!("{}", r);
        }
    } else {
        println!("Error running tests: {}", res.unwrap_err());
    }
}
