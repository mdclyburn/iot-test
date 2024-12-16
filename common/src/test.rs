//! Defining and executing tests

use std::cmp::{Ord, Ordering, PartialOrd, Reverse};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::error;
use std::fmt;
use std::fmt::Display;
use std::iter::IntoIterator;
use std::time::{Duration, Instant};

use rppal::gpio::{
    self,
    Gpio,
    InputPin,
    Level,
    Trigger,
};
use rppal::uart::{self, Uart};

use crate::comm::Signal;
use crate::criteria::{
    Criterion,
    GPIOCriterion,
};
use crate::facility::EnergyMetering;
use crate::io::{DeviceInputs, DeviceOutputs, IOError};
use crate::mem::MemoryTrace;

type Result<T> = std::result::Result<T, TestingError>;

/// Testing error.
#[derive(Debug)]
pub enum TestingError {
    /// GPIO-related error.
    GPIO(gpio::Error),
    /// Testbed to device I/O error.
    IO(IOError),
    /// Energy meter does not exist.
    NoSuchMeter(String),
    /// Invalid test protocol data received.
    Protocol,
    /// Reset requested when [`io::Mapping`] does not specify one.
    Reset(IOError),
    /// Error configuring UART hardware.
    UART(uart::Error),
}

impl error::Error for TestingError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use TestingError::*;
        match self {
            GPIO(ref e) => Some(e),
            IO(ref e) => Some(e),
            Protocol => None,
            Reset(ref e) => Some(e),
            UART(ref e) => Some(e),
            _ => None,
        }
    }
}

impl From<IOError> for TestingError {
    fn from(e: IOError) -> Self {
        TestingError::IO(e)
    }
}

impl From<gpio::Error> for TestingError {
    fn from(e: gpio::Error) -> Self {
        TestingError::GPIO(e)
    }
}

impl From<uart::Error> for TestingError {
    fn from(e: uart::Error) -> Self {
        TestingError::UART(e)
    }
}

impl Display for TestingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use TestingError::*;
        match self {
            GPIO(ref e) => write!(f, "GPIO error while testing: {}", e),
            IO(ref e) => write!(f, "I/O error: {}", e),
            NoSuchMeter(ref id) => write!(f, "the meter '{}' does not exist", id),
            Protocol => write!(f, "testbed/DUT test protocol mismatch"),
            Reset(ref e) => write!(f, "failed to reset device: {}", e),
            UART(ref e) => write!(f, "UART configuration error: {}", e),
        }
    }
}

/// An action that occurs as part of an operation.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Action {
    /// No-op
    Idle(Duration),
    /// Apply an input signal to a particular pin.
    Input(Signal, u8),
}

impl Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Action::*;
        match self {
            Idle(d) => write!(f, "idle for {:?}", d),
            Input(signal, pin) => write!(f, "input {}, pin {}", signal, pin),
        }
    }
}

/// An input to perform at a specific time.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Operation {
    time: u64,
    action: Option<Action>,
}

impl Operation {
    /// Specify the time for the Operation to occur.
    pub fn at(time: u64) -> Operation {
        Operation {
            time,
            // Create with no action initially.
            // Idling should be explicit and not accidental/coincidental
            // since there is a function to create idle time.
            // Empty Operations should be ignored by testing.
            action: None,
        }
    }

    /// Create an input.
    pub fn input(self, signal: Signal, pin: u8) -> Self {
        Self {
            action: Some(Action::Input(signal, pin)),
            ..self
        }
    }

    /// Create a synchronous idling period.
    pub fn idle_sync(self, length: Duration) -> Self {
        Self {
            action: Some(Action::Idle(length)),
            ..self
        }
    }
}

impl Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let action_text = if let Some(action) = self.action {
            format!("{}", action)
        } else {
            "None".to_string()
        };

        write!(f, "@{}ms\taction: {}", self.time, action_text)
    }
}

impl Ord for Operation {
    fn cmp(&self, b: &Self) -> Ordering {
        self.time.cmp(&b.time)
    }
}

impl PartialOrd for Operation {
    fn partial_cmp(&self, b: &Self) -> Option<Ordering> {
        self.time.partial_cmp(&b.time)
    }
}

/// An output response from the device under test.
#[derive(Copy, Clone, Debug)]
pub struct Response {
    time: Instant,
    pin_no: u8,
    output: Signal,
}

impl Response {
    /// Create a new Response.
    pub fn new(time: Instant, pin_no: u8, output: Signal) -> Response {
        Response {
            time,
            pin_no,
            output,
        }
    }

    /// Returns the Instant the Response was recorded.
    pub fn get_time(&self) -> Instant {
        self.time
    }

    /** Obtain the amount of time between a fixed point and the occurence of this Response.

    This is typically used to get the point in time during a test a response occured.
     */
    pub fn get_offset(&self, t0: Instant) -> Duration {
        if self.time > t0 {
            self.time - t0
        } else {
            Duration::from_millis(0)
        }
    }

    /// Returns the pin number the response occurred on.
    pub fn get_pin(&self) -> u8 {
        self.pin_no
    }

    /// Returns the output signal of the response.
    pub fn get_output(&self) -> Signal {
        self.output
    }

    /// Translates the pin numbering to the target-side numbering.
    pub fn remapped(&self, host_target_map: &HashMap<u8, u8>) -> Response {
        let target_pin = host_target_map.get(&self.pin_no)
            .expect("Cannot remap device response because pin mapping does not exist.");

        Response {
            pin_no: *target_pin,
            .. *self
        }
    }
}

impl Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "response on P{:02} {}", self.pin_no, self.output)
    }
}

/// Test execution information
#[derive(Clone, Debug)]
pub struct Execution {
    started_at: Instant,
    finished_at: Instant,
}

impl Execution {
    /// Create a new Execution.
    fn new(started_at: Instant, finished_at: Instant) -> Execution {
        Execution {
            started_at,
            finished_at,
        }
    }

    /// Return the point in time the test execution started.
    pub fn get_start(&self) -> Instant {
        self.started_at
    }

    /// Return the length of time the test ran for.
    pub fn duration(&self) -> Duration {
        self.finished_at - self.started_at
    }
}

/** Test definition.

A test mainly consists of a timeline of [`Operation`]s to perform (inputs to the device under test)
and a set of responses ([`Criterion`]) to record (outputs from the device under test).

Executing a test (via [`Test::execute`]) produces an [`Execution`] that contains information about the test run.

 */
#[derive(Clone, Debug)]
pub struct Test {
    id: String,
    app_ids: HashSet<String>,
    trace_points: HashSet<String>,
    actions: BinaryHeap<Reverse<Operation>>,
    criteria: Vec<Criterion>,
    tail_duration: Option<Duration>,
    reset_device: bool,
}

impl Test {
    /// Define a new test.
    pub fn new<'a, T, U, V, W>(id: &str,
                               app_id: T,
                               trace_points: U,
                               ops: V,
                               criteria: W,
                               reset_device: bool) -> Test
    where
        T: IntoIterator<Item = &'a str>,
        U: IntoIterator<Item = &'a str>,
        V: IntoIterator<Item = &'a Operation>,
        W: IntoIterator<Item = &'a Criterion>,
    {
        Test {
            id: id.to_string(),
            app_ids: app_id.into_iter().map(|id| id.to_string()).collect(),
            trace_points: trace_points.into_iter().map(|tp| tp.to_string()).collect(),
            actions: ops.into_iter().map(|x| Reverse(*x)).collect(),
            criteria: criteria.into_iter().cloned().collect(),
            tail_duration: Some(Duration::from_millis(5)),
            reset_device,
        }
    }

    /// Returns the identifier of the test definition.
    pub fn get_id(&self) -> &str {
        &self.id
    }

    /// Returns the identifiers of the applications the test exercises.
    pub fn get_app_ids(&self) -> &HashSet<String> {
        &self.app_ids
    }

    /// Returns the trace points the test requires.
    pub fn get_trace_points(&self) -> &HashSet<String> {
        &self.trace_points
    }

    /// Returns defined test criteria.
    pub fn get_criteria(&self) -> &Vec<Criterion> {
        &self.criteria
    }

    /// Returns true if the device under test should reset for the test.
    pub fn get_reset_on_start(&self) -> bool {
        self.reset_device
    }

    /// Drive test outputs (inputs to the device).
    pub fn execute(&self, t0: Instant, pins: &mut DeviceInputs) -> Result<Execution> {
        let timeline = self.actions.iter()
            .map(|Reverse(op)| (t0 + Duration::from_millis(op.time), op));
        for (t, op) in timeline {
            while Instant::now() < t {  } // spin wait?

            if let Some(action) = op.action {
                match action {
                    Action::Idle(wait_length) => {
                        let t_until = t + wait_length;
                        while Instant::now() < t_until {  } // spin wait?
                    },

                    Action::Input(signal, pin_no) => match signal {
                        Signal::Digital(true) => (*pins.get_pin_mut(pin_no)?).set_high(),
                        Signal::Digital(false) => (*pins.get_pin_mut(pin_no)?).set_low(),
                        input => panic!("Unhandled input type: {:?}", input),
                    },
                };
            }
        }

        Ok(Execution::new(t0, Instant::now()))
    }

    /// Set up to record test inputs.
    pub fn prep_observe(&self,
                        pins: &mut DeviceOutputs) -> Result<Vec<u8>>
    {
        let mut interrupt_pins: Vec<u8> = Vec::new();

        let gpio_criteria = self.criteria.iter()
            .filter_map(|criterion| {
                if let Criterion::GPIO(gpio_crit) = criterion {
                    Some(gpio_crit)
                } else {
                    None
                }
            });
        for criterion in gpio_criteria {
            println!("observer: watching for {}", criterion);
            match criterion {
                GPIOCriterion::Any(pin_no) => {
                    pins.get_pin_mut(*pin_no)?
                        .set_interrupt(Trigger::Both)?;
                    interrupt_pins.push(*pin_no);
                },
            };
        }

        Ok(interrupt_pins)
    }

    /// Record test inputs (outputs from the device).
    ///
    /// Watches for responses from the device under test for a slightly longer duration than the duration of the test.
    /// This is done to catch any straggling responses from the device.
    pub fn observe(&self,
                   t0: Instant,
                   pins: &Vec<&InputPin>,
                   out: &mut Vec<Response>) -> Result<()>
    {
        let gpio = Gpio::new()?;
        let t_end = t0 + self.max_runtime();
        let mut t = Instant::now();

        while t < t_end {
            let poll = gpio.poll_interrupts(
                pins.as_slice(),
                false,
                Some(t_end - t))?;

            if let Some((pin, level)) = poll {
                let response = Response::new(
                    Instant::now(),
                    pin.pin(),
                    match level {
                        Level::High => Signal::Digital(true),
                        Level::Low => Signal::Digital(false),
                    });
                out.push(response);
            }

            t = Instant::now();
        }

        Ok(())
    }

    /// Prepare structures for energy metering.
    ///
    /// # Returns
    /// Returns true if there are energy metering criteria in this test.
    /// [`Test::meter`] should be called when running the test.
    pub fn prep_meter(&self,
                      meters: &HashMap<String, Box<dyn EnergyMetering>>,
                      out: &mut HashMap<String, Vec<(Instant, f32)>>,
    ) -> Result<bool> {
        // only care about meters defined in the criteria
        out.clear();

        let approx_loop_micros = 545;
        let max_sample_count = (self.max_runtime().as_micros() /
                                approx_loop_micros as u128) + 1;

        let mut has_energy_criteria = false;
        // pre-allocate space in sample output vectors
        for criterion in &self.criteria {
            if let Criterion::Energy(ref energy_criterion) = criterion {
                has_energy_criteria = true;
                let meter_id = energy_criterion.get_meter();
                if !meters.contains_key(meter_id) {
                    return Err(TestingError::NoSuchMeter(meter_id.to_string()));
                } else {
                    out.entry(meter_id.to_string())
                        .or_insert(Vec::new())
                        .reserve_exact(max_sample_count as usize);
                }
            }
        }

        Ok(has_energy_criteria)
    }

    /// Perform energy metering.
    ///
    /// The `out` parameter should be the same `out` passed to [`Test::prep_meter`].
    pub fn meter(&self,
                 meters: &HashMap<String, Box<dyn EnergyMetering>>,
                 out: &mut HashMap<String, Vec<(Instant, f32)>>)
    {
        let start = Instant::now();
        let runtime = self.max_runtime();

        // Without the call to thread::sleep, a single loop iteration
        // takes between 545.568us and 699.682us, averages 568.521us.
        // Reading a single meter in this loop yields me 94 samples.
        // So, sampling interval is actually: self.energy_sampling_rate + ~.5ms.
        // It makes sense to lose about 5 out of 100 samples for
        // self.energy_sampling_rate = 10ms given a test that executes for
        // 1000ms. 1000ms / 10.5ms/samples = 95.238 samples.
        // let mut ra: f32 = 0.0;
        // let threshold = Duration::from_millis(1000);
        loop {
            let now = Instant::now();
            let d_test = now - start;

            if d_test >= runtime { break; }

            for (id, buf) in &mut *out {
                let meter = meters.get(id).unwrap();
                let p = meter.power();
                // if p < 20.0 && d_test > threshold { panic!(); }
                // if p > 97.0 { continue; }
                // ra = (ra * 0.99) + (p * 0.01);
                // buf.push((now, if buf.len() > 500 { ra } else { p }));
                buf.push((now, p));
            }
        }
    }

    /// Prepare structures for tracing.
    pub fn prep_tracing<'a>(&self,
                            uart: &mut Uart,
                            data_buffer: &'a mut Vec<u8>,
                            schedule: &'a mut Vec<(Instant, usize)>) -> Result<()> {
        // Timeout is a bit arbitrary here.
        // Don't want the thread hanging the test unnecessarily.
        uart.set_read_mode(0, Duration::from_millis(50))?;

        schedule.clear();

        let buffer_alloc: usize = 1 * 1024 * 1024;
        data_buffer.reserve_exact(buffer_alloc);
        schedule.reserve_exact(buffer_alloc);
        while data_buffer.len() < buffer_alloc { data_buffer.push(0); }
        // Clear out any early data that arrives before the reset.
        uart.flush(rppal::uart::Queue::Input)?;

        Ok(())
    }

    /// Perform the tracing specified by the test.
    pub fn trace(&self,
                 uart: &mut Uart,
                 buffer: &mut Vec<u8>,
                 schedule: &mut Vec<(Instant, usize)>) -> Result<usize> {
        let buffer: &mut [u8] = buffer.as_mut_slice();
        let mut bytes_read: usize = 0;

        let max_runtime = self.max_runtime();
        let start = Instant::now();

        loop {
            let now = Instant::now();
            if now - start >= max_runtime { break; }

            let read = uart.read(&mut buffer[bytes_read..])?;
            if read > 0 {
                bytes_read += read;
                schedule.push((now, read));
            }
        }

        Ok(bytes_read)
    }

    /// Prepare structures for memory tracking.
    pub fn prep_memtrack(&self,
                         uart: &mut Uart,
                         buffer: &mut Vec<u8>,
                         schedule: &mut Vec<MemoryTrace>) -> Result<()>
    {
        // Again, timeout is arbitrary.
        uart.set_read_mode(0, Duration::from_millis(100))?;

        schedule.clear();

        let buffer_alloc = 1 * 1024 * 1024;
        buffer.reserve_exact(buffer_alloc);
        // Arbitrary estimation...
        schedule.reserve_exact(buffer_alloc / 10);
        while buffer.len() < buffer_alloc { buffer.push(0); }

        Ok(())
    }

    /// Perform memory tracking.
    pub fn memtrack(&self,
                    uart: &mut Uart,
                    buffer: &mut Vec<u8>,
                    schedule: &mut Vec<MemoryTrace>) -> Result<usize>
    {
        let buffer: &mut [u8] = buffer.as_mut_slice();
        let mut bytes_read = 0;

        let max_runtime = self.max_runtime();
        let start = Instant::now();

        let mut buffered_now = start;
        let mut bytes_parsed = 0;

        /* Strategy:
        Read bytes received over UART into buffer.
        Upon reception of data, always note the Instant the data is received.
        Then, try to parse the data into one or more StreamOperations.
        If successful, place the StreamOperation into the schedule vector
        along with the noted time of reception of the first byte.
        Repeat this process until the buffer is fully parsed
        or this strategy yields no more StreamOperations.
        If there is no data remaining in the buffer, then the noted Instant is considered as 'expired'.
        It will not be used for the next sequence of data received.
        If there is data remaining in the buffer, there is a StreamOperation that hasn't finished traversing the wire
        and we must hold the prior noted Instant to attach to this incoming StreamOperation.
         */

        loop {
            let now = Instant::now();
            if now - start >= max_runtime { break; }

            // Check if we still have data we must parse.
            // If we do, we cannot discard the buffered Instant yet
            // because we still have data received around that time.
            //
            // Not updating the buffered Instant does mean that data
            // can appear to arrive earlier than it really did.
            // This is a drawback of parsing these on-demand instead of
            // afterwards.
            if bytes_parsed < bytes_read {
                buffered_now = now;
            }

            let read = uart.read(&mut buffer[bytes_read..])?;
            if read > 0 {
                bytes_read += read;

                // Try to parse the stream operations.
                while bytes_parsed < bytes_read {
                    use crate::mem::parse_counter as parse_mem_counter;
                    use nom::Err as NomError;

                    // The data that needs parsing.
                    let to_parse = &buffer[bytes_parsed..bytes_read];
                    match parse_mem_counter(to_parse, buffered_now) {
                        // Parser successfully read a stream operation.
                        // We advance our bytes_parsed marker forward by the number of bytes we parsed.
                        Ok(((unparsed, _bit_offset), op)) => {
                            schedule.push(op);
                            bytes_parsed += to_parse.len() - unparsed.len();
                        },
                        Err(ref nom_error) => match nom_error {
                            // Parser ran out of bytes in the middle of parsing.
                            // This is fine and expected to happen at times.
                            // We break out of this loop and try to read more data from UART.
                            NomError::Incomplete(_need) => break,
                            // Parser tried to parse data and it didn't understand.
                            // We can't recover from this at this level (yet).
                            NomError::Failure(_parse_error) => return Err(TestingError::Protocol),
                            // Parser should not return an Error to us.
                            NomError::Error(parse_error) => {
                                let mut msg: String = format!("Temporary parser error surfaced.\nThis is a bug. Check byte offset {}. Buffer:\n",
                                                              bytes_parsed);
                                for (col, byte) in (0..8).cycle().zip(&buffer[0..bytes_read]) {
                                    msg.push_str(&format!("{:#04X}{}", byte, if col == 7 { '\n' } else { ' ' }));
                                }
                                panic!("{}\nError: {:?}", msg, parse_error);
                            }
                        }
                    };
                }
            }
        }

        println!("memtrack: bytes rx: {}, bytes parsed: {}", bytes_read, bytes_parsed);
        Ok(bytes_read - bytes_parsed)
    }

    /// Return the maximum length of time the test can run.
    ///
    /// TODO: make this dependent on actions' timing, criteria timing, and another tail duration(?).
    pub fn max_runtime(&self) -> Duration {
        let duration_ms = self.actions.iter()
            // Only Operations with actions.
            .filter(|Reverse(op)| op.action.is_some())
            .map(|Reverse(op)| match op.action.unwrap() {
                Action::Idle(idle_duration) => op.time + (idle_duration.as_millis() as u64),
                _ => op.time,
            })
            .last()
            .unwrap_or(0);
        let tail_ms = self.tail_duration
            .unwrap_or(Duration::from_millis(0))
            .as_millis();

        Duration::from_millis(duration_ms + tail_ms as u64)
    }
}

impl Display for Test {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Test: {}\n", self.id)?;
        write!(f, "=== Operation timeline\n")?;
        write!(f, "|{:>10}|{:^20}|\n", "time (ms)", "operation")?;
        write!(f, "|----------+--------------------|\n")?;
        for Reverse(ref action) in &self.actions {
            if let Some(act) = action.action {
                let act_text = format!("{}", act);
                write!(f, "|{:>10}|{:^20}|\n", action.time, act_text)?;
            } else {
                write!(f, "|{:>10}|{:^20}|\n", action.time, "-")?;
            }
        }
        write!(f, "\n")?;

        write!(f, "=== Criteria\n")?;
        for criterion in &self.criteria {
            write!(f, "- {}\n", criterion)?;
        }

        Ok(())
    }
}
