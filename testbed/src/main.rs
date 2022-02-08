//! IoT testing tool

use std::process;

mod input;
mod opts;
mod output;

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
    print!("{}\n", testbed);

    let mut tests = configuration.get_test_adapter().tests();
    let observations = testbed.execute(&mut tests);
    for o in &observations {
        println!("{:?}", o);
    }
}
