use std::fmt;
use std::fmt::Display;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::device::Device;
use crate::io::{IOPin, Mapping};
use crate::testing::{Test, Evaluation, Status};

#[derive(Debug)]
pub struct Testbed<'a> {
    dut: &'a Device,
    pin_mapping: &'a Mapping,
}

impl<'a> Testbed<'a> {
    pub fn new(device: &'a Device, pin_mapping: &'a Mapping) -> Testbed<'a> {
        Testbed {
            dut: device,
            pin_mapping: pin_mapping,
        }
    }

    pub fn execute<T, U>(&self, tests: T, out: &mut U) where
        T: IntoIterator<Item = &'a Test>,
        U: Extend<Evaluation> {
        let mut test_results = Vec::new();

        let watch_ready = Arc::new(RwLock::new(false));
        let ready = Arc::new(RwLock::new(false));
        let current_test: Arc<RwLock<Option<&Test>>> = Arc::new(RwLock::new(None));

        println!("Starting response watch thread.");
        let watch_thread = {
            let ready = Arc::clone(&watch_ready);
            let executor_ready = Arc::clone(&ready);

            thread::spawn(move || {
                println!("Response watch thread started.");

                // wait for executor to be ready
                while ! *executor_ready.read().unwrap() {
                    println!("Waiting for testbed to signal ready.");
                    thread::sleep(Duration::from_millis(50));
                }

                // prepare to watch current test

                // signal readiness and wait for start time
                *watch_ready.write().unwrap() = true;
            })
        };

        for test in tests {
            *current_test.write().unwrap() = Some(test);
            test_results.push(Evaluation::new("bah", Status::Invalid));
        }

        watch_thread.join().unwrap(); // need to go to testbed error

        out.extend(test_results);
    }
}

impl<'a> Display for Testbed<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Testbed\n{}", self.pin_mapping)
    }
}
