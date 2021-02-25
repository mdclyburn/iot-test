mod device;
mod io;
mod testing;
mod testbed;

use crate::device::{Device, IODirection};
use crate::io::Mapping;
use crate::testbed::Testbed;
use crate::testing::{Test, Operation, Signal};

fn main() {
    let device = Device::new(&[(2, (IODirection::In, device::Signal::Digital))]);
    let mapping = Mapping::new(&device, &[(17, 2)]).unwrap();
    let testbed = Testbed::new(&device, &mapping);
    print!("{}\n\n", testbed);

    let test = Test::new(
        "first one",
        &[Operation { time: 0, input: Signal::High(2) },
          Operation { time: 500, input: Signal::Low(2) }],
        &[]);

    print!("{}\n\n", test);

    let mut results = Vec::new();
    testbed.execute(&[test], &mut results);
    for r in results {
        println!("{}", r);
    }
}
