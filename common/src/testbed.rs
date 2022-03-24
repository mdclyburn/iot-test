//! Configure and execute tests.

use std::collections::HashMap;
use std::error;
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
use crate::io::{IOError, Mapping, UART};
use crate::mem::MemoryTrace;
use crate::output::DataWriter;
use crate::sw::{self, PlatformSupport};
use crate::sw::instrument::Spec;
use crate::test::{Execution, Response, Test, TestingError};
use crate::trace;
use crate::trace::{TraceData, TraceKind, SerialTrace};

// Errors that originate within the testbed code should map to a relevant TestbedError.
type Result<T> = std::result::Result<T, TestbedError>;

/// Errors that occur while the testbed is running.
#[derive(Debug)]
pub enum TestbedError {
    /// A problem occured while executing a test.
    Execution(TestingError),
    /// A problem occured while performing a reset operation on the device.
    Reset(IOError),
    /// A problem occured while interacting with software ([`sw::error::Error`]).
    Software(sw::error::SoftwareError),
}

impl error::Error for TestbedError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use TestbedError::*;
        match self {
            Execution(ref e) => Some(e),
            Reset(ref e) => Some(e),
            Software(ref e) => Some(e),
        }
    }
}

impl From<TestingError> for TestbedError {
    fn from(e: TestingError) -> Self {
        TestbedError::Execution(e)
    }
}

impl From<sw::error::SoftwareError> for TestbedError {
    fn from(e: sw::error::SoftwareError) -> TestbedError {
        TestbedError::Software(e)
    }
}

impl Display for TestbedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use TestbedError::*;
        match self {
            Execution(ref e) => write!(f, "test execution error: {}", e),
            Reset(ref e) => write!(f, "failed to reset device: {}", e),
            Software(ref e) => write!(f, "software interaction error: {}", e),
        }
    }
}

/// Test suite executor
#[derive(Debug)]
pub struct Testbed {
    pin_mapping: Mapping,
    platform_support: Box<dyn PlatformSupport>,
    energy_meters: Arc<Mutex<HashMap<String, Box<dyn EnergyMetering>>>>,
    tracing_uart: Option<UART>,
    memory_uart: Option<UART>,
    tracing: Vec<(TraceKind, UART)>,
    data_writer: Option<Box<dyn DataWriter>>,
}

impl Testbed {
    /// Create a new `Testbed`.
    pub fn new(
        pin_mapping: Mapping,
        platform_support: Box<dyn PlatformSupport>,
        energy_meters: HashMap<String, Box<dyn EnergyMetering>>,
        tracing_uart: Option<UART>,
        memory_uart: Option<UART>,
        tracing: Vec<(TraceKind, UART)>,
    ) -> Testbed
    {
        Testbed {
            pin_mapping,
            platform_support,
            energy_meters: Arc::new(Mutex::new(energy_meters)),
            tracing_uart,
            memory_uart,
            tracing,
            data_writer: None,
        }
    }

    /// Define a write for testing data.
    pub fn save_results_with(&mut self, formatter: Box<dyn DataWriter>) {
        self.data_writer = Some(formatter);
    }

    /** Run tests.

    Execute the given tests one after the other.

    # Examples
    ```
    let mut results = Vec::new();
    testbed.execute(&[test], &mut results);
    ```
     */
    pub fn execute<'b, T>(&self, tests: &mut T) -> Vec<Observation>
    where
        T: Iterator<Item = Test>,
    {
        let mut test_results = Vec::new();

        let barrier = {
            let barrier_count =
            // One for each staticly allocated thread we have:
            // - Main testbed thread
            // - Observer thread
            // - Energy metering thread
            // - Serial tracing thread
            // - Memory tracing thread
                5
            // One for each user-defined tracing thread
                + self.tracing.len();

            Arc::new(Barrier::new(barrier_count))
        };

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
                                               trace_schannel,
                                               self.tracing_uart.as_ref());

        let (mem_schannel, mem_rchannel) = mpsc::sync_channel(0);
        let mem_thread = self.launch_memstat(Arc::clone(&current_test),
                                             Arc::clone(&barrier),
                                             mem_schannel,
                                             self.memory_uart.as_ref());

        // Create threads for the defined tracing purposes.
        // Keep track of the receiving ends of their channels.
        // The ordering must be consistent between the two vectors.
        let tracing_rchannels = {
            let mut rchannels = Vec::new();
            for (kind, uart) in &self.tracing {
                let (schannel, rchannel) = mpsc::sync_channel(0);
                self.launch_tracing_kind(
                    kind.clone(),
                    uart,
                    Arc::clone(&current_test),
                    Arc::clone(&barrier),
                    schannel);
                rchannels.push(rchannel);
            }

            rchannels
        };

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
                let observation = Observation::failed(
                    test.clone(),
                    None,
                    TestbedError::Software(reconfig_err));
                test_results.push(observation);
                continue;
            }
            let platform_spec = res.unwrap();

            // Load application(s) if necessary.
            if let Err(load_err) = self.load_apps(&test) {
                println!("executor: error loading/removing application(s)");
                let observation = Observation::failed(
                    test.clone(),
                    Some(platform_spec.clone()),
                    load_err);
                test_results.push(observation);
                continue;
            }

            *current_test.write().unwrap() = Some(test.clone());

            let mut inputs = self.pin_mapping.get_gpio_inputs()
                .expect("Could not obtain GPIO inputs from executor thread.");

            // wait for observer, metering thread to be ready
            barrier.wait();

            let use_reset = test.get_reset_on_start();
            if use_reset {
                println!("Placing device in reset.");
                let reset_res = self.pin_mapping.get_device().hold_in_reset(&mut inputs);
                if let Err(e) = reset_res {
                    let observation = Observation::failed(
                        test.clone(),
                        Some(platform_spec.clone()),
                        TestbedError::Reset(e));
                    test_results.push(observation);
                    continue;
                }
            }

            // wait for test to begin
            barrier.wait();
            println!("executor: starting test '{}'", test.get_id());

            // make sure testing has _just_ started before releasing reset
            if use_reset {
                self.pin_mapping.get_device().release_from_reset(&mut inputs)
                    // failed to release reset, no point in continuing
                    .expect("failed to release device from reset");
            }
            let exec_result = test.execute(Instant::now(), &mut inputs)
                .map_err(|e| TestbedError::Execution(e));

            // release observer thread
            println!("executor: test execution complete");
            barrier.wait();

            // get GPIO responses
            let mut gpio_activity = Vec::new();
            while let Some(response) = observer_rchannel.recv().unwrap() {
                let response = response.remapped(self.pin_mapping.get_mapping());
                gpio_activity.push(response);
            }

            // get energy data
            let mut energy_data = HashMap::new();
            while let Some((meter_id, (t, sample))) = energy_rchannel.recv().unwrap() {
                energy_data.entry(meter_id)
                    .or_insert(Vec::new())
                    .push((t, sample));
            }

            // get tracing data
            println!("executor: receiving trace data");
            let mut serial_traces: Vec<SerialTrace> = Vec::new();
            while let Some(trace) = trace_rchannel.recv().unwrap() {
                serial_traces.push(trace);
            }

            let start = exec_result.as_ref().map(|exec| exec.get_start()).unwrap();
            for trace in &serial_traces {
                println!("{} @ {:?}", trace, trace.get_offset(start));
            }

            // get memory data
            println!("executor: receiving memory data");
            let mut mem_traces: Vec<MemoryTrace> = Vec::new();
            println!("| {:^15} | op. | {:^35} | {:^6} |", "offset", "counter", "value");
            while let Some(mem_event) = mem_rchannel.recv().unwrap() {
                let offset = format!("@{:?}", mem_event.time() - exec_result.as_ref().unwrap().get_start());
                let counter = format!("{}", mem_event.counter());
                println!("| {:>15} | {:^5?} | {:^35} | {:>6} |",
                         offset,
                         mem_event.operation(),
                         counter,
                         mem_event.value());
                mem_traces.push(mem_event);
            }

            // Receive tracing data.
            let mut trace_data = Vec::new();
            let iter = tracing_rchannels.iter()
                .zip(self.tracing.iter());
            for (rchannel, (trace_kind, _uart)) in iter {
                println!("executor: receiving data from {} thread", trace_kind);
                let data = rchannel.recv()
                    .expect("Failed to receive data from tracing channel.");
                trace_data.push(data);
            }

            // save data
            if let (Some(writer), Ok(execution)) = (self.data_writer.as_ref(), exec_result.as_ref()) {
                println!("executor: sending test data to writer");
                writer.save_output(
                    &test,
                    execution,
                    &gpio_activity,
                    &serial_traces,
                    &energy_data)
                    .expect("failed to save test data");
            }

            let observation = Observation::completed(
                test.clone(),
                Some(platform_spec.clone()),
                exec_result,
                gpio_activity,
                serial_traces,
                self.tracing.iter()
                    .map(|(kind, _uart)| kind)
                    .collect(),
                trace_data,
                energy_data);
            test_results.push(observation);
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
        mem_thread.join().unwrap_or_else(|_e| {
            println!("executor: failed to join with memory thread");
        });

        test_results
    }

    fn launch_observer(
        &self,
        test_container: Arc<RwLock<Option<Test>>>,
        barrier: Arc<Barrier>,
        response_schannel: SyncSender<Option<Response>>,
    ) -> JoinHandle<()> {
        let mut outputs = self.pin_mapping.get_gpio_outputs()
            .expect("Could not obtain GPIO outputs from observer thread.");

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
                        let interrupt_pin_nos = test.prep_observe(&mut outputs)
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
        energy_schannel: SyncSender<Option<(String, (Instant, f32))>>,
    ) -> JoinHandle<()> {
        println!("Starting energy metering thread.");

        let meters = Arc::clone(&self.energy_meters);

        thread::Builder::new()
            .name("test-metering".to_string())
            .spawn(move || {
                println!("metering: started.");

                let meters = meters.lock().unwrap();
                let mut samples: HashMap<String, Vec<(Instant, f32)>> = meters.keys()
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
        uart: Option<&UART>,
    ) -> JoinHandle<()> {

        if let Some(uart) = uart {
            println!("Starting tracing thread.");
            let uart = self.pin_mapping.get_uart(uart)
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
        } else {
            println!("No UART for serial tracing; will idle.");

            thread::Builder::new()
                .name("test-stracing".to_string())
                .spawn(move || {

                    loop {
                        // wait for next test
                        barrier.wait();
                        if let Some(ref _test) = *test_container.read().unwrap() {
                            barrier.wait();
                        } else {
                            // no more tests to run
                            break;
                        }
                        barrier.wait();
                        trace_schannel.send(None).unwrap(); // done communicating results
                    }
                })
                .expect("Could not spawn tracing thread.")
        }
    }

    fn launch_memstat(
        &self,
        test_container: Arc<RwLock<Option<Test>>>,
        barrier: Arc<Barrier>,
        mem_schannel: SyncSender<Option<MemoryTrace>>,
        uart: Option<&UART>,
    ) -> JoinHandle<()> {
        if let Some(uart) = uart {
            println!("Starting memory tracking thread.");
            let uart = self.pin_mapping.get_uart(uart)
                .expect("Could not obtain UART from tracing thread.");

            thread::Builder::new()
                .name("test-memtrack".to_string())
                .spawn(move || {
                    println!("memtrack: started.");

                    let mut uart = uart;
                    let mut buffer: Vec<u8> = Vec::new();
                    let mut schedule: Vec<MemoryTrace> = Vec::new();
                    let mut bytes_remaining;

                    loop {
                        // wait for next test
                        barrier.wait();

                        if let Some(ref test) = *test_container.read().unwrap() {
                            test.prep_memtrack(&mut uart, &mut buffer, &mut schedule).unwrap();

                            barrier.wait();
                            bytes_remaining = test.memtrack(
                                &mut uart,
                                &mut buffer,
                                &mut schedule).unwrap();
                            if bytes_remaining > 0 {
                                println!("memtrack: {} bytes of unprocessed data!", bytes_remaining);
                            } else {
                                println!("memtrack: all data processed");
                            }
                        } else {
                            // no more tests to run
                            break;
                        }

                        barrier.wait();

                        for mem_event in &schedule {
                            mem_schannel.send(Some(mem_event.clone())).unwrap();
                        }
                        mem_schannel.send(None).unwrap(); // done communicating results
                    }
                })
                .expect("Could not spawn memory tracking thread.")
        } else {
            println!("No UART for memory tracking; will idle.");

            thread::Builder::new()
                .name("test-memtrack".to_string())
                .spawn(move || {

                    loop {
                        // wait for next test
                        barrier.wait();
                        if let Some(ref _test) = *test_container.read().unwrap() {
                            barrier.wait();
                        } else {
                            // no more tests to run
                            break;
                        }
                        barrier.wait();
                        mem_schannel.send(None).unwrap(); // done communicating results
                    }
                })
                .expect("Could not spawn tracing thread.")
        }
    }

    fn launch_tracing_kind(
        &self,
        kind: TraceKind,
        uart: &UART,
        test_container: Arc<RwLock<Option<Test>>>,
        barrier: Arc<Barrier>,
        schannel: SyncSender<Option<TraceData>>,
    ) -> JoinHandle<()> {
        let name = format!("test-{}", kind);
        let mut uart = self.pin_mapping.get_uart(uart)
            .expect("Could not obtain UART for tracing.");

        thread::Builder::new()
            .name(name.clone())
            .spawn(move || {
                println!("trace-{}: starting", &name);

                let mut buffer: Vec<u8> = Vec::new();
                let mut uart = uart;
                let mut trace_data = None;

                loop {
                    // Wait for next test.
                    barrier.wait();

                    if let Some(ref test) = *test_container.read().unwrap() {
                        // Prepare for testing.
                        // Break out allocating the space in the buffer prior to actually running testing
                        // to minimize any jitter between the barrier and the collection starting.
                        let prepared_buffer = trace::prepare(&mut buffer, &mut uart)
                            .unwrap();

                        barrier.wait();
                        let t_stop_at = Instant::now() + test.max_runtime();
                        trace_data = match trace::collect(&kind, &mut uart, prepared_buffer, t_stop_at) {
                            Ok(trace_data) => Some(trace_data),
                            Err(e) => {
                                println!("trace-{}: tracing for {} failed: {}", name, kind, e);
                                None
                            },
                        };
                    } else {
                        // No more tests to run.
                        break;
                    }

                    // Post-testing wait.
                    barrier.wait();

                    // Send data back.
                    schannel.send(trace_data).expect("failed to send trace data to main thread");
                }
            }).expect("Could not spawn tracing thread.")
    }

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
                    .map_err(|e| TestbedError::Software(e))?;
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

/// Aggregated collection of test execution data.
#[derive(Debug)]
pub struct Observation<'a> {
    test: Test,
    software_spec: Option<Spec>,
    execution_result: Result<Execution>,
    gpio_responses: Vec<Response>,
    traces: Vec<SerialTrace>,
    trace_info: Vec<&'a TraceKind>,
    trace_data: Vec<Option<TraceData>>,
    energy_metrics: HashMap<String, Vec<(Instant, f32)>>,
}

impl<'a> Observation<'a> {
    fn completed(
        test: Test,
        software_spec: Option<Spec>,
        execution_result: Result<Execution>,
        gpio_responses: Vec<Response>,
        traces: Vec<SerialTrace>,
        trace_info: Vec<&'a TraceKind>,
        trace_data: Vec<Option<TraceData>>,
        energy_metrics: HashMap<String, Vec<(Instant, f32)>>
    ) -> Observation<'a> {
        Observation {
            test,
            software_spec,
            execution_result,
            gpio_responses,
            traces,
            trace_info,
            trace_data,
            energy_metrics,
        }
    }

    fn failed(
        test: Test,
        software_spec: Option<Spec>,
        error: TestbedError,
    ) -> Observation<'a> {
        Observation {
            test,
            software_spec,
            execution_result: Err(error),
            gpio_responses: Vec::new(),
            traces: Vec::new(),
            trace_info: Vec::new(),
            trace_data: Vec::new(),
            energy_metrics: HashMap::new(),
        }
    }

    /// Return the test that the `Observation` is for.
    pub fn source_test(&self) -> &Test {
        &self.test
    }

    /// Return the execution metadata of running the test against the device.
    pub fn execution_result(&self) -> &Result<Execution> {
        &self.execution_result
    }

    /// Return the software configuration used for the test.
    pub fn software_config(&self) -> Option<&Spec> {
        self.software_spec.as_ref().clone()
    }

    /// Return GPIO state changes observed during the test.
    pub fn gpio_responses(&self) -> &Vec<Response> {
        &self.gpio_responses
    }

    /// Return the traces received from the device during the test.
    pub fn traces(&self) -> &Vec<SerialTrace> {
        &self.traces
    }

    /// Return data from all energy meters active during the test.
    pub fn energy_metrics(&self) -> &HashMap<String, Vec<(Instant, f32)>> {
        &self.energy_metrics
    }
}
