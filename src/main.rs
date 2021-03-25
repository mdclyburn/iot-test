mod comm;
mod device;
mod facility;
mod hw;
mod io;
mod testing;

use crate::comm::{Direction, Class as SignalClass, Signal};
use crate::device::Device;
use crate::hw::INA219;
use crate::hw::hal::ADC;
use crate::io::Mapping;
use crate::testing::{Criterion, Test, Testbed, Operation};

fn main() {
    let device = Device::new(&[
        (13, (Direction::Out, SignalClass::Digital)),
        (23, (Direction::In, SignalClass::Digital)),
    ]);
    let mapping = Mapping::new(&device, &[(17, 23), (2, 13)]).unwrap();

    let ina219 = INA219::new(
        mapping.get_i2c().unwrap(),
        0b10000000)
        .unwrap();
    let _emr: &dyn facility::EnergyMetering = &ina219;
    let testbed = Testbed::new(&mapping);
    print!("{}\n\n", testbed);

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
