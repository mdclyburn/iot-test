//! Configuring tests and executing tests

use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::sync::mpsc;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc,
                Barrier,
                Mutex,
                RwLock};
use std::thread;
use std::thread::JoinHandle;
use std::time::Instant;

use crate::facility::EnergyMetering;
use crate::io::Mapping;
use crate::sw::{Loadable, Platform};
use crate::sw::application::ApplicationSet;
use crate::testing::test::Response;

use super::{Error, Evaluation, Test};

type Result<T> = std::result::Result<T, Error>;

/// Test suite executor
#[derive(Debug)]
pub struct Testbed {
    pin_mapping: Mapping,
    energy_meters: Arc<Mutex<HashMap<String, Box<dyn EnergyMetering>>>>,
    platform_configs: HashMap<Platform, Box<dyn Loadable>>,
    applications: Option<ApplicationSet>
}

impl Testbed {
    /// Create a new `Testbed`.
    pub fn new<'a, T, U>(pin_mapping: Mapping,
                         energy_meters: T,
                         platform_configs: U,
                         applications: Option<ApplicationSet>) -> Testbed
    where
        T: IntoIterator<Item = (&'a str, Box<dyn EnergyMetering>)>,
        U: IntoIterator<Item = Box<dyn Loadable>>,
    {
        let energy_meters = energy_meters.into_iter()
            .map(|(id, meter)| (id.to_string(), meter))
            .collect();

        Testbed {
            pin_mapping,
            energy_meters: Arc::new(Mutex::new(energy_meters)),
            platform_configs: platform_configs.into_iter()
                .map(|config| (config.platform(), config))
                .collect(),
            applications,
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
    pub fn execute<'a, T>(&self, tests: T) -> Result<Vec<Evaluation>> where
        T: IntoIterator<Item = &'a Test>
    {
        let mut test_results = Vec::new();

        let barrier = Arc::new(Barrier::new(3));
        let current_test: Arc<RwLock<Option<Test>>> = Arc::new(RwLock::new(None));

        let (test_result_schannel, rchannel) = mpsc::sync_channel(0);
        let watch_thread = self.launch_observer(Arc::clone(&current_test),
                                                Arc::clone(&barrier),
                                                test_result_schannel)?;

        let (energy_schannel, energy_rchannel) = mpsc::sync_channel(0);
        let energy_thread = self.launch_metering(Arc::clone(&current_test),
                                                 Arc::clone(&barrier),
                                                 energy_schannel)?;

        for test in tests {
            // load application if necessary


            *current_test.write().unwrap() = Some(test.clone());

            let mut inputs = self.pin_mapping.get_gpio_inputs()?;

            // wait for observer, metering thread to be ready
            barrier.wait();

            // wait for test to begin
            barrier.wait();
            println!("executor: starting test '{}'", test.get_id());

            let exec_result = test.execute(Instant::now(), &mut inputs);

            // release observer thread
            println!("executor: test execution complete");
            barrier.wait();

            // get GPIO responses
            let mut responses = Vec::new();
            while let Some(response) = rchannel.recv()? {
                responses.push(response);
            }

            // get energy data
            let mut energy_data = HashMap::new();
            while let Some((meter_id, sample)) = energy_rchannel.recv()? {
                energy_data.entry(meter_id)
                    .or_insert(Vec::new())
                    .push(sample);
            }

            test_results.push(Evaluation::new(test, exec_result, responses, energy_data));
            println!("executor: test finished.");
        }

        *current_test.write().unwrap() = None;
        println!("executor: final wait");
        barrier.wait();

        watch_thread.join().unwrap(); // need to go to testbed error
        energy_thread.join().unwrap(); // uh... you need to handle these

        Ok(test_results)
    }

    fn launch_observer(
        &self,
        test_container: Arc<RwLock<Option<Test>>>,
        barrier: Arc<Barrier>,
        response_schannel: SyncSender<Option<Response>>,
    ) -> Result<JoinHandle<()>> {
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
                    if let Some(ref test) = *test_container.read().unwrap() {
                        test.prep_observe(&mut outputs)
                            .unwrap(); // <-- communicate back?

                        // wait for test to begin
                        println!("observer: ready to begin test");
                        barrier.wait();
                        println!("observer: starting watch");

                        let t0 = Instant::now();
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
                            response_schannel.send(Some(r)).unwrap();
                        }
                        response_schannel.send(None).unwrap();
                    } else {
                        // no more tests to run
                        break;
                    }
                }

                println!("observer: exiting");
            })
            .map_err(|e| Error::Threading(e))
    }

    fn launch_metering(
        &self,
        test_container: Arc<RwLock<Option<Test>>>,
        barrier: Arc<Barrier>,
        energy_schannel: SyncSender<Option<(String, f32)>>,
    ) -> Result<JoinHandle<()>> {
        println!("Starting energy metering thread.");

        let meters = Arc::clone(&self.energy_meters);

        thread::Builder::new()
            .name("test-metering".to_string())
            .spawn(move || {
                println!("metering: started.");

                let meters = meters.lock().unwrap();
                let mut samples: HashMap<String, Vec<f32>> = meters.keys()
                    .map(|meter_id| { (meter_id.clone(), Vec::new()) })
                    .collect();

                loop {
                    // wait for next test
                    barrier.wait();

                    if let Some(ref test) = *test_container.read().unwrap() {
                        // here, better error management across threads would be nice!
                        let need_metering = test.prep_meter(&meters, &mut samples).unwrap();
                        if !need_metering {
                            println!("metering: idling; not needed for this test");
                            barrier.wait();
                        } else {
                            // wait for test to begin
                            println!("metering: ready to begin test");
                            barrier.wait();

                            test.meter(&meters, &mut samples);
                        }
                    } else {
                        // no more tests to run
                        break;
                    }

                    barrier.wait();

                    // communicate results back
                    for (meter_id, samples) in &samples {
                        for sample in samples {
                            // .to_string()... kinda wasteful, but it works;
                            // perhaps better comm. types wanted?
                            let message = Some((meter_id.to_string(), *sample));
                            energy_schannel.send(message).unwrap();
                        }
                    }
                    energy_schannel.send(None).unwrap(); // done communicating results
                }
            })
            .map_err(|e| Error::Threading(e))
    }

    // fn load_app(&self, test: &Test) -> Result<()> {
    // }
}

impl Display for Testbed {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Testbed\n{}", self.pin_mapping)?;

        write!(f, "\nEnergy meters:\n")?;
        if let Ok(meters) = self.energy_meters.lock() {
            for meter_id in meters.keys() {
                write!(f, " - '{}'\n", meter_id)?;
            }
        } else {
            write!(f, " (unavailable)\n")?;
        }

        Ok(())
    }
}
