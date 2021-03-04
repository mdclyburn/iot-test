//! Configuring tests and executing tests

use std::fmt;
use std::fmt::Display;
use std::sync::mpsc;
use std::sync::{Arc, Barrier, RwLock};
use std::thread;
use std::time::Instant;

use crate::io::Mapping;

use super::{Error, Evaluation, Test};

type Result<T> = std::result::Result<T, Error>;

/// Test suite executor
#[derive(Debug)]
pub struct Testbed<'a> {
    // TODO: reference? why?
    pin_mapping: &'a Mapping,
}

impl<'a> Testbed<'a> {
    /// Create a new `Testbed`.
    pub fn new(pin_mapping: &'a Mapping) -> Testbed<'a> {
        Testbed {
            pin_mapping,
        }
    }

    /** Run tests.
     *
     * Execute the given tests one after the other.
     *
     * # Examples
     * ```
     *    let mut results = Vec::new();
     *    testbed.execute(&[test], &mut results);
     * ```
     */
    pub fn execute<T>(&self, tests: T) -> Result<Vec<Evaluation>> where
        T: IntoIterator<Item = &'a Test>
    {
        let mut test_results = Vec::new();

        let barrier = Arc::new(Barrier::new(2));
        let current_test: Arc<RwLock<Option<Test>>> = Arc::new(RwLock::new(None));
        let watch_start: Arc<RwLock<Option<Instant>>> = Arc::new(RwLock::new(None));

        let (schannel, rchannel) = mpsc::sync_channel(0);

        println!("Starting response watch thread.");
        let watch_thread = {
            let barrier = Arc::clone(&barrier);
            let current_test = Arc::clone(&current_test);
            let watch_start = Arc::clone(&watch_start);
            let mut outputs = self.pin_mapping.get_outputs()?;

            thread::spawn(move || {
                println!("watcher: started.");

                let mut responses = Vec::new();
                loop {
                    // wait for next test
                    barrier.wait();

                    // set up to watch for responses according to criteria
                    if let Some(ref test) = *current_test.read().unwrap() {
                        test.prep_observe(&mut outputs)
                            .unwrap(); // <-- communicate back?

                        // wait for test to begin
                        println!("watcher: ready to begin test");
                        barrier.wait();
                        let t0 = Instant::now();
                        *watch_start.write().unwrap() = Some(t0);
                        println!("watcher: starting watch");

                        test.observe(t0, &outputs, &mut responses)
                            .unwrap();

                        // wait for output responses from dut or the end of the test
                        // can I just wait for the barrier here or will an interrupt stop it?
                        barrier.wait();

                        // TODO: clear interrupts

                        for r in responses.drain(..) {
                            schannel.send(Some(r)).unwrap();
                        }
                        schannel.send(None).unwrap();
                    } else {
                        // no more tests to run
                        break;
                    }
                }

                println!("watcher: exiting");
            })
        };

        let mut launching_at: Option<Instant> = None;
        for test in tests {
            *current_test.write().unwrap() = Some(test.clone());

            let mut inputs = self.pin_mapping.get_inputs()?;

            // wait for watcher thread to be ready
            barrier.wait();
            launching_at = Some(Instant::now());

            // wait for test to begin
            barrier.wait();
            println!("executor: starting test '{}'", test.get_id());

            let exec_result = test.execute(launching_at.unwrap(), &mut inputs);

            // release watcher thread
            println!("executor: test execution complete");
            barrier.wait();

            // get responses to build an Evaluation
            let mut responses = Vec::new();
            while let Some(response) = rchannel.recv()? {
                responses.push(response);
            }

            test_results.push(Evaluation::new(test, exec_result, responses));
            println!("executor: test finished.");
        }

        *current_test.write().unwrap() = None;
        println!("executor: final wait");
        barrier.wait();

        watch_thread.join().unwrap(); // need to go to testbed error

        let main_start = launching_at.unwrap();
        let watch_starta = watch_start.read().unwrap().unwrap();
        let desync = if main_start > watch_starta {
            main_start - watch_starta
        } else {
            watch_starta - main_start
        };
        println!("threads de-synced on time by {:?}", desync);

        Ok(test_results)
    }
}

impl<'a> Display for Testbed<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Testbed\n{}", self.pin_mapping)
    }
}
