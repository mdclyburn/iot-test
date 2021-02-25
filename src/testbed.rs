use std::fmt;
use std::fmt::Display;
use std::sync::mpsc;
use std::sync::{Arc, Barrier, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::device::Device;
use crate::io::Mapping;
use crate::testing;
use crate::testing::{Test, Execution, Response};

#[derive(Copy, Clone, Debug)]
pub enum Status {
    Complete,
    Pass,
    Fail,
    Error,
}

impl Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Status::Complete => write!(f, "Complete"),
            Status::Pass => write!(f, "Pass"),
            Status::Fail => write!(f, "Fail"),
            Status::Error => write!(f, "Error"),
        }
    }
}

#[derive(Debug)]
pub struct Evaluation {
    test_id: String,
    exec_result: Result<Execution, testing::Error>,
}

impl Evaluation {
    pub fn new(test: &Test, exec_result: Result<Execution, testing::Error>) -> Evaluation {
        Evaluation {
            test_id: test.get_id().to_string(),
            exec_result,
        }
    }

    pub fn get_outcome(&self) -> Status {
        if self.exec_result.is_err() {
            Status::Error
        } else {
            // more nuanced info here
            Status::Complete
        }
    }

    pub fn get_exec_result(&self) -> &Result<Execution, testing::Error> {
        &self.exec_result
    }
}

impl Display for Evaluation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}\t", self.test_id)?;
        match self.get_outcome() {
            Status::Error => write!(f, "Error ({})", self.get_exec_result().as_ref().unwrap_err()),
            outcome => write!(f, "{} (in {:?})", outcome, self.get_exec_result().as_ref().unwrap().get_duration()),
        }
    }
}

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
        let current_test: Arc<RwLock<Option<Test>>> = Arc::new(RwLock::new(None));
        let watch_start: Arc<RwLock<Option<Instant>>> = Arc::new(RwLock::new(None));

        let (schannel, rchannel) = mpsc::sync_channel(0);

        println!("Starting response watch thread.");
        let watch_thread = {
            let barrier = Arc::clone(&barrier);
            let current_test = Arc::clone(&current_test);
            let watch_start = Arc::clone(&watch_start);
            let dut_output = Arc::clone(self.pin_mapping.get_outputs());

            thread::spawn(move || {
                println!("watcher: started.");

                let dut_output = dut_output.lock().unwrap();
                let mut responses: Vec<Response> = Vec::new();
                loop {
                    // wait for next test
                    barrier.wait();

                    // set up to watch for responses according to criteria
                    if let Some(ref test) = *current_test.read().unwrap() {
                        for c in test.get_criteria() {
                            match c {
                                _ => println!("watcher: don't know how to watch {:?}", c)
                            }
                        }
                    } else {
                        // no more tests to run
                        break;
                    }

                    // wait for test to begin
                    println!("watcher: ready to begin test");
                    barrier.wait();
                    *watch_start.write().unwrap() = Some(Instant::now());

                    println!("watcher: starting watch");

                    // wait for output responses from dut or the end of the test
                    // can I just wait for the barrier here or will an interrupt stop it?
                    barrier.wait();

                    for r in responses.drain(..) {
                        schannel.send(Some(r)).unwrap();
                    }
                    schannel.send(None).unwrap();
                }

                println!("watcher: exiting");
            })
        };

        let mut launching_at: Option<Instant> = None;
        for test in tests {
            *current_test.write().unwrap() = Some(test.clone());

            let inputs = self.pin_mapping.get_inputs().lock().unwrap();

            // wait for watcher thread to be ready
            barrier.wait();
            launching_at = Some(Instant::now());

            // wait for test to begin
            barrier.wait();
            println!("executor: starting test '{}'", test.get_id());

            let exec_result = test.execute(launching_at.unwrap(), &inputs);

            // release watcher thread
            println!("executor: test execution complete");
            barrier.wait();

            // get responses to build an Evaluation
            while let Some(response) = rchannel.recv().unwrap() {
                // build evaluation with this information.
            }

            test_results.push(Evaluation::new(test, exec_result));
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

        out.extend(test_results);
    }
}

impl<'a> Display for Testbed<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Testbed\n{}", self.pin_mapping)
    }
}
