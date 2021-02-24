use std::fmt;
use std::fmt::Display;
use std::sync::mpsc;
use std::sync::{Arc, Barrier, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::device::Device;
use crate::io::{IOPin, Mapping};
use crate::testing::{Test, Evaluation, Response, Status};

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

        let barrier = Arc::new(Barrier::new(2));
        let current_test: Arc<RwLock<Option<&Test>>> = Arc::new(RwLock::new(None));
        let watch_start: Arc<RwLock<Option<Instant>>> = Arc::new(RwLock::new(None));
        let (schannel, _rchannel) = mpsc::sync_channel(0);

        println!("Starting response watch thread.");
        let watch_thread = {
            let barrier = Arc::clone(&barrier);
            let current_test = Arc::clone(&current_test);
            let watch_start = Arc::clone(&watch_start);

            thread::spawn(move || {
                println!("watcher: started.");

                // prepare to watch current test
                let mut responses: Vec<Response> = Vec::new();

                // wait for test to begin
                barrier.wait();
                println!("watcher: starting watch");
                *watch_start.write().unwrap() = Some(Instant::now());

                for r in &responses {
                    schannel.send(*r).unwrap();
                }
            })
        };

        let mut launching_at: Option<Instant> = None;
        for test in tests {
            *current_test.write().unwrap() = Some(test);

            // wait for watcher thread to be ready
            barrier.wait();

            println!("executor: starting");
            launching_at = Some(Instant::now());

            test_results.push(Evaluation::new("bah", Status::Invalid));
            println!("Test complete.");
        }

        watch_thread.join().unwrap(); // need to go to testbed error

        let main_start = launching_at.unwrap();
        let watch_starta = watch_start.read().unwrap().unwrap();
        let desync = if main_start > watch_starta {
            main_start - watch_starta
        } else {
            watch_starta - main_start
        };
        println!("threads de-synced on time by {:?}", desync);

        out.extend(test_results);
    }
}

impl<'a> Display for Testbed<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Testbed\n{}", self.pin_mapping)
    }
}
