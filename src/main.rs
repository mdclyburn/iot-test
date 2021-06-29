//! IoT testing tool

use std::collections::HashMap;
use std::path::Path;
use std::process;
use std::time::Duration;

mod comm;
mod device;
mod facility;
mod hw;
mod input;
mod io;
mod opts;
mod sw;
mod testing;

use crate::comm::{Direction, Class as SignalClass, Signal};
use crate::device::Device;
use crate::facility::EnergyMetering;
use crate::hw::INA219;
use crate::input::TestbedConfigReader;
use crate::input::json::JSONTestbedParser;
use crate::io::Mapping;
use crate::sw::application::{Application, ApplicationSet};
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

fn main() {
    let result = opts::parse();
    if let Err(ref e) = result {
        use opts::Error::*;
        match e {
            Help(msg) => println!("{}", msg),
            _ => println!("Initialization failed.\n{}", e),
        };
        process::exit(1);
    }
    let configuration = result.unwrap();

    let result = configuration.get_testbed_reader().create();
    if let Err(ref e) = result {
        println!("Failed to initialize testbed.\n{}", e);
        process::exit(1);
    }
    let testbed = result.unwrap();

    // // physical mapping
    // let device = Device::new(
    //     &[(13, (Direction::Out, SignalClass::Digital)), // D0
    //       (14, (Direction::Out, SignalClass::Digital)), // D1
    //       (19, (Direction::Out, SignalClass::Digital)), // D6
    //       (20, (Direction::Out, SignalClass::Digital)), // D7
    //       (23, (Direction::In, SignalClass::Digital)),  // reset
    //     ]);
    // let mapping = Mapping::new(
    //     &device,
    //     &[(17, 23), // reset

    //       // GPIO tracing
    //       (14, 13),
    //       (15, 14),
    //       (18, 19),
    //       (23, 20),
    //     ],
    //     &[13, 14, 19, 20],
    // ).unwrap();

    // // energy metering
    // let ina219 = INA219::new(mapping.get_i2c().unwrap(), 0x40)
    //     .unwrap();
    // let energy_meters: Vec<(&str, Box<dyn EnergyMetering>)> = vec![
    //     ("system", Box::new(ina219))
    // ];
    // let energy_meters: HashMap<String, _> = energy_meters.into_iter()
    //     .map(|(name, meter)| (name.to_string(), meter))
    //     .collect();

    // // platform support
    // let tock_support = Tock::new(
    //     Path::new("/usr/local/bin/tockloader"),
    //     Path::new("/home/ubuntu/work/tock"));

    // // applications
    // let app_set = ApplicationSet::new(
    //     &[Application::new("blink", &[(Platform::Tock, Path::new("/home/ubuntu/work/apps/tock/blink.tab"))])]
    // );

    // let testbed = Testbed::new(
    //     mapping,
    //     Box::new(tock_support),
    //     energy_meters,
    //     Some(app_set));
    // print!("{}\n", testbed);
    println!("{}\n", testbed);
    return;

    let tests = [
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

        Test::new(
            "blink-trace-alpha",
            (&[]).into_iter().copied(),
            (&["capsule/led/command/on", "capsule/led/command/off"]).into_iter().copied(),
            &[Operation { time: 0, pin_no: 23, input: Signal::Digital(false) },
              Operation { time: 3000, pin_no: 23, input: Signal::Digital(true) }],
            &[Criterion::Trace(TraceCriterion::new(&[TraceCondition::new(2).with_extra_data(1),
                                                     TraceCondition::new(1).with_timing(Timing::Relative(Duration::from_millis(50)),
                                                                                        Duration::from_millis(5))
                                                     .with_extra_data(1)]))])
    ];

    for test in &tests {
        print!("{}\n\n", test);
    }

                            println!("  timing of event matches");
    let res = testbed.execute(&tests);
    if let Ok(results) = res {
        for r in results {
            println!("{}", r);
        }
    } else {
        println!("Error running tests: {}", res.unwrap_err());
    }
}
