//! IoT testing tool

use std::process;

mod comm;
mod device;
mod facility;
mod hw;
mod input;
mod io;
mod opts;
mod sw;
mod testing;

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
    print!("{}\n", testbed);

    let tests: Vec<Test> = configuration.get_test_adapter().tests()
        .into_iter()
        .map(|r| r.unwrap().clone())
        .collect();

    let res = testbed.execute(&tests);
    if let Ok(results) = res {
        for r in results {
            println!("{}", r);
        }
    } else {
        println!("Error running tests: {}", res.unwrap_err());
    }
}
