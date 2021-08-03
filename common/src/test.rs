//! Defining and executing tests

use std::cmp::{Ord, Ordering, PartialOrd, Reverse};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fmt;
use std::fmt::Display;
use std::iter::IntoIterator;
use std::time::{Duration, Instant};

use rppal::gpio::{
    Gpio,
    InputPin,
    Level,
    Trigger,
};
use rppal::uart::Uart;

use crate::Error;
use crate::comm::Signal;
use crate::criteria::{
    Criterion,
    GPIOCriterion,
};
use crate::facility::EnergyMetering;
use crate::io::{DeviceInputs, DeviceOutputs};

type Result<T> = std::result::Result<T, Error>;

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

    pub fn get_pin(&self) -> u8 {
        self.pin_no
    }

    pub fn get_output(&self) -> Signal {
        self.output
    }

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
    pub fn get_start(&self) -> &Instant {
        &self.started_at
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
                        pins: &mut DeviceOutputs,
                        trace_pins: &Vec<u8>) -> Result<Vec<u8>>
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

        // Configure interrupts on the trace pins differently if specified.
        let contains_trace_criterion = self.criteria.iter()
            .find(|c| match c {
                Criterion::ParallelTrace(_) => true,
                _ => false,
            })
            .is_some();
        if contains_trace_criterion {
            for pin_no in trace_pins {
                // Last pin triggers on both to signal final trace pin change.
                let trigger = if *pin_no == trace_pins[trace_pins.len()-1] {
                    Trigger::Both
                } else {
                    Trigger::RisingEdge
                };

                println!("observer: configuring trace pin {}", pin_no);
                pins.get_pin_mut(*pin_no)?
                    .set_interrupt(trigger)?;
            }
        }

        // Always check trace pins first in their provided order.
        let trace_ins = trace_pins.into_iter().zip(0..);
        for (&pin_no, pos) in trace_ins {
            interrupt_pins.insert(pos, pin_no);
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
        let t_end = t0 + self.get_max_runtime();
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
                      out: &mut HashMap<String, Vec<f32>>,
    ) -> Result<bool> {
        // only care about meters defined in the criteria
        out.clear();

        let approx_loop_micros = 545;
        let max_sample_count = (self.get_max_runtime().as_micros() /
                                approx_loop_micros as u128) + 1;

        let mut has_energy_criteria = false;
        // pre-allocate space in sample output vectors
        for criterion in &self.criteria {
            if let Criterion::Energy(ref energy_criterion) = criterion {
                has_energy_criteria = true;
                let meter_id = energy_criterion.get_meter();
                if !meters.contains_key(meter_id) {
                    return Err(Error::NoSuchMeter(meter_id.to_string()));
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
    pub fn meter(&self, meters: &HashMap<String, Box<dyn EnergyMetering>>, out: &mut HashMap<String, Vec<f32>>) {
        let start = Instant::now();
        let runtime = self.get_max_runtime();

        // Without the call to thread::sleep, a single loop iteration
        // takes between 545.568us and 699.682us, averages 568.521us.
        // Reading a single meter in this loop yields me 94 samples.
        // So, sampling interval is actually: self.energy_sampling_rate + ~.5ms.
        // It makes sense to lose about 5 out of 100 samples for
        // self.energy_sampling_rate = 10ms given a test that executes for
        // 1000ms. 1000ms / 10.5ms/samples = 95.238 samples.
        loop {
            if Instant::now() - start >= runtime { break; }

            for (id, buf) in &mut *out {
                let meter = meters.get(id).unwrap();
                buf.push(meter.power());
            }
        }
    }

    pub fn prep_tracing<'a>(&self,
                            uart: &mut Uart,
                            data_buffer: &'a mut Vec<u8>,
                            schedule: &'a mut Vec<(Instant, usize)>) -> Result<()> {
        // Timeout is a bit arbitrary here.
        // Don't want the thread hanging the test unnecessarily.
        uart.set_read_mode(0, Duration::from_millis(100))?;

        schedule.clear();

        let buffer_alloc: usize = 1 * 1024 * 1024;
        data_buffer.reserve_exact(buffer_alloc);
        schedule.reserve_exact(buffer_alloc);
        while data_buffer.len() < buffer_alloc { data_buffer.push(0); }

        Ok(())
    }

    pub fn trace(&self,
                 uart: &mut Uart,
                 buffer: &mut Vec<u8>,
                 schedule: &mut Vec<(Instant, usize)>) -> Result<usize> {
        let buffer: &mut [u8] = buffer.as_mut_slice();
        let mut bytes_read: usize = 0;

        let max_runtime = self.get_max_runtime();
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

    /// Return the maximum length of time the test can run.
    ///
    /// TODO: make this dependent on actions' timing, criteria timing, and another tail duration(?).
    fn get_max_runtime(&self) -> Duration {
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