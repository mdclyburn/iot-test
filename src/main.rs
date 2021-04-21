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
use crate::sw::Platform;
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
    let device = Device::new(&[
        (13, (Direction::Out, SignalClass::Digital)),
        (23, (Direction::In, SignalClass::Digital)),
    ]);
    let mapping = Mapping::new(&device, &[(17, 23), (27, 13)]).unwrap();

    // energy metering
    let ina219 = INA219::new(mapping.get_i2c().unwrap(), 0x40)
        .unwrap();
    let energy_meters: Vec<(&str, Box<dyn EnergyMetering>)> = vec![("system", Box::new(ina219))];

    // applications
    let app_set = ApplicationSet::new(
        &[Application::new("blink", &[(Platform::Tock, Path::new("/home/ubuntu/work/apps/tock/blink.tab"))])]
    );

    let testbed = Testbed::new(
        mapping,
        energy_meters,
        Some(app_set));
    print!("{}\n", testbed);

    let test = Test::new(
        "example-blink-test",
        &[Operation { time: 0, pin_no: 23, input: Signal::Digital(true) },
          Operation { time: 500, pin_no: 23, input: Signal::Digital(false) }],
        &[Criterion::GPIO(GPIOCriterion::Any(13)),
          Criterion::Energy(EnergyCriterion::new("system", EnergyStat::Total))
        ]);
    let tests = [test];

    print!("{}\n\n", tests[0]);

    let res = testbed.execute(&tests);
    if let Ok(results) = res {
        for r in results {
            println!("{}", r);
        }
    } else {
        println!("Error running tests: {}", res.unwrap_err());
    }
}
