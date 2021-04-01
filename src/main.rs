mod comm;
mod device;
mod facility;
mod hw;
mod io;
mod testing;

use crate::comm::{Direction, Class as SignalClass, Signal};
use crate::device::Device;
use crate::facility::EnergyMetering;
use crate::hw::INA219;
use crate::io::Mapping;
use crate::testing::{Criterion, Test, Testbed, Operation};

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
    println!("INA219 voltage: {:.2}V", ina219.bus_voltage().unwrap());
    println!("INA219 wattage: {:.2}mW", ina219.power().unwrap());
    let energy_meters: Vec<(&str, Box<dyn EnergyMetering>)> = vec![("system", Box::new(ina219))];

    let testbed = Testbed::new(
        &mapping,
        energy_meters);
    print!("{}\n\n", testbed);
    // std::process::exit(0);

    let test = Test::new(
        "example-blink-test",
        &[Operation { time: 0, pin_no: 23, input: Signal::Digital(true) },
          Operation { time: 500, pin_no: 23, input: Signal::Digital(false) }],
        &[Criterion::Response(13)]);
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
