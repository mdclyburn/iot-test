mod device;
mod io;
mod testing;

use crate::device::{Device, IODirection};
use crate::io::Mapping;
use crate::testing::{Criterion, Test, Testbed, Operation, Signal};

fn main() {
    let device = Device::new(&[
        (13, (IODirection::Out, device::Signal::Digital)),
        (23, (IODirection::In, device::Signal::Digital)),
    ]);
    let mapping = Mapping::new(&device, &[(17, 23), (2, 13)]).unwrap();
    let testbed = Testbed::new(&device, &mapping);
    print!("{}\n\n", testbed);

    let test = Test::new(
        "first one",
        &[Operation { time: 0, input: Signal::High(23) },
          Operation { time: 500, input: Signal::Low(23) }],
        &[Criterion::Response(13)]);

    print!("{}\n\n", test);

    let mut res = testbed.execute(&[test]);
    if let Ok(results) = res {
        for r in results {
            println!("{:?}", r);
        }
    } else {
        println!("Error running tests: {}", res.unwrap_err());
    }
}
