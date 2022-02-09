//! IoT testing tool

use std::process;

use clockwise_common::evaluation::{Evaluator, StandardEvaluator};

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

    // Use the evaluator to produce results from collected data.
    // Here we only use the StandardEvaluator for now.
    let evaluator = StandardEvaluator::new();
    let evaluation_iter = observations.iter()
        .map(|obs| evaluator.evaluate(obs));
    for evaluation in evaluation_iter {
        println!("{}", evaluation);
    }
}
