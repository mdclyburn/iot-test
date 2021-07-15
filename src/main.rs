//! IoT testing tool

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

use crate::comm::Signal;
use crate::testing::criteria::{
    Criterion,
    // GPIOCriterion,
    // EnergyCriterion,
    // EnergyStat,
    Timing,
    ParallelTraceCondition,
    ParallelTraceCriterion,
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
    print!("{}\n", testbed);

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
            &[Criterion::ParallelTrace(ParallelTraceCriterion::new(&[ParallelTraceCondition::new(2).with_extra_data(1),
                                                     ParallelTraceCondition::new(1).with_timing(Timing::Relative(Duration::from_millis(50)),
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
