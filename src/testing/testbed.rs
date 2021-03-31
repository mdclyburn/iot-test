//! Configuring tests and executing tests

use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::sync::mpsc;
use std::sync::{Arc,
                Barrier,
                Mutex,
                RwLock};
use std::thread;
use std::time::Instant;

use crate::facility::EnergyMetering;
use crate::io::Mapping;

use super::{Error, Evaluation, Test};

type Result<T> = std::result::Result<T, Error>;

/// Test suite executor
#[derive(Debug)]
pub struct Testbed<'a> {
    // TODO: reference? why?
    pin_mapping: &'a Mapping,
    energy_meters: Arc<Mutex<HashMap<String, Box<dyn EnergyMetering>>>>,
}

impl<'a> Testbed<'a> {
    /// Create a new `Testbed`.
    pub fn new<'b, T>(pin_mapping: &'a Mapping, energy_meters: T) -> Testbed<'a>
    where
        T: IntoIterator<Item = (&'b str, Box<dyn EnergyMetering>)>
    {
        let energy_meters = energy_meters.into_iter()
            .map(|(id, meter)| (id.to_string(), meter))
            .collect();

        Testbed {
            pin_mapping,
            energy_meters: Arc::new(Mutex::new(energy_meters)),
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

        let barrier = Arc::new(Barrier::new(3));
        let current_test: Arc<RwLock<Option<Test>>> = Arc::new(RwLock::new(None));
        let watch_start: Arc<RwLock<Option<Instant>>> = Arc::new(RwLock::new(None));

        let (schannel, rchannel) = mpsc::sync_channel(0);

        println!("Starting response watch thread.");
        let watch_thread = {
            let barrier = Arc::clone(&barrier);
            let current_test = Arc::clone(&current_test);
            let watch_start = Arc::clone(&watch_start);
            let mut outputs = self.pin_mapping.get_gpio_outputs()?;

            thread::Builder::new()
                .name("test-observer".to_string())
                .spawn(move || {
                    println!("observer: started.");

                    let mut responses = Vec::new();
                    loop {
                        // wait for next test
                        barrier.wait();

                        // set up to watch for responses according to criteria
                        if let Some(ref test) = *current_test.read().unwrap() {
                            test.prep_observe(&mut outputs)
                                .unwrap(); // <-- communicate back?

                            // wait for test to begin
                            println!("observer: ready to begin test");
                            barrier.wait();
                            let t0 = Instant::now();
                            *watch_start.write().unwrap() = Some(t0);
                            println!("observer: starting watch");

                            test.observe(t0, &outputs, &mut responses)
                                .unwrap();

                            // wait for output responses from dut or the end of the test
                            // can I just wait for the barrier here or will an interrupt stop it?
                            barrier.wait();

                            println!("observer: cleaning up interrupts");
                            for pin in &mut outputs {
                                pin.clear_interrupt().unwrap();
                            }

                            for r in responses.drain(..) {
                                schannel.send(Some(r)).unwrap();
                            }
                            schannel.send(None).unwrap();
                        } else {
                            // no more tests to run
                            break;
                        }
                    }

                    println!("observer: exiting");
                })
                .map_err(|e| Error::Observer(e))?
        };

        println!("Starting energy metering thread.");
        let energy_thread = {
            let barrier = Arc::clone(&barrier);
            let current_test = Arc::clone(&current_test);
            let meters = Arc::clone(&self.energy_meters);

            thread::Builder::new()
                .name("test-metering".to_string())
                .spawn(move || {
                    println!("metering: started.");

                    loop {
                        let meters = meters.lock().unwrap();

                        // wait for next test
                        barrier.wait();

                        if let Some(ref test) = *current_test.read().unwrap() {
                            // wait for test to begin
                            println!("metering: ready to begin test");
                            barrier.wait();
                        }

                        println!("metering: starting metering");

                        // do metering

                        for (id, meter) in &(*meters) {
                            let draw = meter.current_draw();
                            println!("metering: {} says {:016b} ({})", id, draw, draw);
                        }

                        barrier.wait();

                        // communicate results back
                    }
                })
        };

        let mut launching_at: Option<Instant> = None;
        for test in tests {
            *current_test.write().unwrap() = Some(test.clone());

            let mut inputs = self.pin_mapping.get_gpio_inputs()?;

            // wait for observer thread to be ready
            barrier.wait();
            launching_at = Some(Instant::now());

            // wait for test to begin
            barrier.wait();
            println!("executor: starting test '{}'", test.get_id());

            let exec_result = test.execute(launching_at.unwrap(), &mut inputs);

            // release observer thread
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
