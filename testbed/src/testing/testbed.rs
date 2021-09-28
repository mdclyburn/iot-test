//! Configure and execute tests.

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

use flexbed_common::facility::EnergyMetering;
use flexbed_common::io::Mapping;
use flexbed_common::mem::StreamOperation;
use flexbed_common::test::{Response, Test};
use flexbed_common::trace;
use flexbed_common::trace::SerialTrace;

use crate::sw::PlatformSupport;

use super::Error;
use super::evaluation::Evaluation;

type Result<T> = std::result::Result<T, Error>;

/// Test suite executor
#[derive(Debug)]
pub struct Testbed {
    pin_mapping: Mapping,
    platform_support: Box<dyn PlatformSupport>,
    energy_meters: Arc<Mutex<HashMap<String, Box<dyn EnergyMetering>>>>,
}

impl Testbed {
    /// Create a new `Testbed`.
    pub fn new(pin_mapping: Mapping,
               platform_support: Box<dyn PlatformSupport>,
               energy_meters: HashMap<String, Box<dyn EnergyMetering>>) -> Testbed
    {
        Testbed {
            pin_mapping,
            platform_support,
            energy_meters: Arc::new(Mutex::new(energy_meters)),
        }
    }

    /** Run tests.

    Execute the given tests one after the other.

    # Examples
    ```
    let mut results = Vec::new();
    testbed.execute(&[test], &mut results);
    ```
     */
    pub fn execute<'b, T>(&self, tests: &mut T) -> Result<Vec<Evaluation>>
    where
        T: Iterator<Item = Test>,
    {
        let mut test_results = Vec::new();

        let barrier = Arc::new(Barrier::new(4));
        let current_test: Arc<RwLock<Option<Test>>> = Arc::new(RwLock::new(None));

        let (observer_schannel, observer_rchannel) = mpsc::sync_channel(0);
        let watch_thread = self.launch_observer(Arc::clone(&current_test),
                                                Arc::clone(&barrier),
                                                observer_schannel);

        let (energy_schannel, energy_rchannel) = mpsc::sync_channel(0);
        let energy_thread = self.launch_metering(Arc::clone(&current_test),
                                                 Arc::clone(&barrier),
                                                 energy_schannel);

        let (trace_schannel, trace_rchannel) = mpsc::sync_channel(0);
        let trace_thread = self.launch_tracing(Arc::clone(&current_test),
                                               Arc::clone(&barrier),
                                               trace_schannel);

        // let (mem_schannel, mem_rchannel) = mpsc::sync_channel(0);
        // let mem_thread =

        for test in tests {
            println!("executor: running '{}'", test.get_id());
            println!("{}", test);

            // Reconfigure target if necessary.
            // Just always configuring when there are trace points
            // instead of doing anything idempotent.
            let trace_points: Vec<String> = test.get_trace_points().iter()
                .cloned()
                .collect();
            let res = self.platform_support.reconfigure(&trace_points);
            if let Err(reconfig_err) = res {
                let eval = Evaluation::failed(
                    &test,
                    None,
                    Error::Software(reconfig_err));
                test_results.push(eval);
                continue;
            }
            let platform_spec = res.unwrap();

            // Load application(s) if necessary.
            if let Err(load_err) = self.load_apps(&test) {
                println!("executor: error loading/removing application(s)");
                let eval = Evaluation::failed(
                    &test,
                    Some(&platform_spec),
                    load_err);
                test_results.push(eval);
                continue;
            }

            *current_test.write().unwrap() = Some(test.clone());

            let mut inputs = self.pin_mapping.get_gpio_inputs()
                .expect("Could not obtain GPIO inputs from executor thread.");

            // wait for observer, metering thread to be ready
            barrier.wait();

            if test.get_reset_on_start() {
                let reset_res = self.pin_mapping.get_device().reset(&mut inputs);
                if let Err(e) = reset_res {
                    test_results.push(
                        Evaluation::failed(
                            &test,
                            Some(&platform_spec),
                            Error::Reset(e)));
                    continue;
                }
            }

            // wait for test to begin
            barrier.wait();
            println!("executor: starting test '{}'", test.get_id());

            // if test.get_reset_on_start() {
            //     let reset_pin_no = self.pin_mapping.get_reset_pin().unwrap();
            //     inputs.get_pin_mut(reset_pin_no).unwrap()
            //         .set_high();
            // }

            let exec_result = test.execute(Instant::now(), &mut inputs)
                .map_err(|e| Error::Execution(e));

            // release observer thread
            println!("executor: test execution complete");
            barrier.wait();

            // get GPIO responses
            let (parallel_traces, gpio_activity) = {
                let mut responses = Vec::new();
                while let Some(response) = observer_rchannel.recv().unwrap() {
                    let response = response.remapped(self.pin_mapping.get_mapping());
                    responses.push(response);
                }

                // Filter for trace responses.
                // Map pin_no -> significance
                let trace_pins: HashMap<u8, u16> = self.pin_mapping.get_trace_pin_nos().iter()
                    .copied()
                    .zip(0..) // bit significance
                    .collect();
                let (traces, all_other): (Vec<Response>, _) = responses.into_iter()
                    .partition(|r| trace_pins.contains_key(&r.get_pin()));
                for r in &traces {
                    println!("TRACE RESPONSE: {} - {:?}",
                             r,
                             r.get_offset(*exec_result.as_ref().unwrap().get_start()));
                }
                let traces = trace::reconstruct_parallel(
                    &traces, &platform_spec, &trace_pins);

                (traces, all_other)
            };

            // get energy data
            let mut energy_data = HashMap::new();
            while let Some((meter_id, sample)) = energy_rchannel.recv().unwrap() {
                energy_data.entry(meter_id)
                    .or_insert(Vec::new())
                    .push(sample);
            }

            // get tracing data
            println!("executor: receiving trace data");
            let mut serial_traces: Vec<SerialTrace> = Vec::new();
            while let Some(trace) = trace_rchannel.recv().unwrap() {
                serial_traces.push(trace);
            }

            let evaluation = Evaluation::new(
                &test,
                &platform_spec,
                exec_result,
                gpio_activity,
                parallel_traces,
                serial_traces,
                energy_data);
            test_results.push(evaluation);
            println!("executor: test finished.");
        }

        *current_test.write().unwrap() = None;
        println!("executor: final wait");
        barrier.wait();

        // Not too concerned with joining these without error
        // since testing is complete at this point. It shouldn't
        // result in a crash either.
        watch_thread.join().unwrap_or_else(|_e| {
            println!("executor: failed to join with observer thread");
        });
        energy_thread.join().unwrap_or_else(|_e| {
            println!("executor: failed to join with metering thread");
        });
        trace_thread.join().unwrap_or_else(|_e| {
            println!("executor: failed to join with tracing thread");
        });

        Ok(test_results)
    }

    fn launch_observer(
        &self,
        test_container: Arc<RwLock<Option<Test>>>,
        barrier: Arc<Barrier>,
        response_schannel: SyncSender<Option<Response>>,
    ) -> JoinHandle<()> {
        let mut outputs = self.pin_mapping.get_gpio_outputs()
            .expect("Could not obtain GPIO outputs from observer thread.");
        let trace_pins = self.pin_mapping.get_trace_pin_nos().clone();

        thread::Builder::new()
            .name("test-observer".to_string())
            .spawn(move || {
                println!("observer: started.");

                let mut responses = Vec::new();
                responses.reserve(1000);
                loop {
                    // wait for next test
                    barrier.wait();

                    // set up to watch for responses according to criteria
                    if let Some(ref test) = *test_container.read().unwrap() {
                        let interrupt_pin_nos = test.prep_observe(&mut outputs, &trace_pins)
                            .unwrap(); // <-- communicate back?
                        let interrupt_pins = interrupt_pin_nos.into_iter()
                            .map(|pin_no| outputs.get_pin(pin_no).unwrap())
                            .collect();

                        // wait for test to begin
                        println!("observer: ready to begin test");
                        barrier.wait();
                        println!("observer: starting watch");

                        let t0 = Instant::now();
                        test.observe(t0, &interrupt_pins, &mut responses)
                            .unwrap();

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
            .expect("Could not spawn observer thread.")
    }

    fn launch_metering(
        &self,
        test_container: Arc<RwLock<Option<Test>>>,
        barrier: Arc<Barrier>,
        energy_schannel: SyncSender<Option<(String, f32)>>,
    ) -> JoinHandle<()> {
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
            .expect("Could not spawn metering thread.")
    }

    fn launch_tracing(
        &self,
        test_container: Arc<RwLock<Option<Test>>>,
        barrier: Arc<Barrier>,
        trace_schannel: SyncSender<Option<SerialTrace>>,
    ) -> JoinHandle<()> {
        println!("Starting tracing thread.");

        let uart = self.pin_mapping.get_uart()
            .expect("Could not obtain UART from tracing thread.");

        thread::Builder::new()
            .name("test-stracing".to_string())
            .spawn(move || {
                println!("stracing: started.");

                let mut uart = uart;
                let mut buffer: Vec<u8> = Vec::new();
                let mut schedule: Vec<(Instant, usize)> = Vec::new();
                let mut bytes_rx;

                loop {
                    // wait for next test
                    barrier.wait();

                    if let Some(ref test) = *test_container.read().unwrap() {
                        test.prep_tracing(&mut uart, &mut buffer, &mut schedule).unwrap();

                        barrier.wait();
                        bytes_rx = test.trace(
                            &mut uart,
                            &mut buffer,
                            &mut schedule).unwrap();
                        println!("stracing: received {} bytes over UART", bytes_rx);
                    } else {
                        // no more tests to run
                        break;
                    }

                    barrier.wait();

                    let serial_traces = trace::reconstruct_serial(
                        &buffer.as_slice()[0..bytes_rx],
                        &schedule);
                    // communicate results back
                    for trace in serial_traces {
                        trace_schannel.send(Some(trace)).unwrap();
                    }
                    trace_schannel.send(None).unwrap(); // done communicating results
                }
            })
            .expect("Could not spawn tracing thread.")
    }

    // fn launch_memory(
    //     &self,
    //     test_container: Arc<RwLock<Option<Test>>>,
    //     barrier: Arc<Barrier>,
    //     mem_schannel: SyncSender<Option<StreamOperation>>,
    // ) -> JoinHandle<()> {
    //     println!("Starting memory thread.");
    // }

    /// Load specified applications onto the device.
    fn load_apps(&self, test: &Test) -> Result<()> {
        println!("executor: loading/unloading {} software", self.platform_support.platform());
        let currently_loaded = self.platform_support.loaded_software();
        for app_id in &currently_loaded {
            if !test.get_app_ids().contains(app_id) {
                println!("executor: removing '{}'", app_id);
                self.platform_support.unload(app_id)?;
            }
        }

        for app_name in test.get_app_ids() {
            if !currently_loaded.contains(app_name) {
                println!("executor: loading '{}'", app_name);
                self.platform_support.load(app_name)
                    .map_err(|e| Error::Software(e))?;
            }
        }

        Ok(())
    }
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
