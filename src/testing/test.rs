//! Defining and executing tests

use std::borrow::Borrow;
use std::cmp::{Ord, Ordering, PartialOrd, Reverse};
use std::collections::BinaryHeap;
use std::convert::From;
use std::error;
use std::fmt;
use std::fmt::Display;
use std::iter::IntoIterator;
use std::time::{Duration, Instant};

use rppal::gpio::{Gpio, InputPin, Level, Trigger};

use crate::io;
use crate::io::{DeviceInputs, DeviceOutputs};
use super::Error;

type Result<T> = std::result::Result<T, Error>;

/// An input signal setting for a particular pin.
#[derive(Copy, Clone, Eq, Debug, PartialEq)]
pub enum Signal {
    /// Digital high
    High(u8),
    /// Digital low
    Low(u8),
}

impl Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Signal::High(pin) => write!(f, "DIGITAL HIGH\tP{:02}", pin),
            Signal::Low(pin) => write!(f, "DIGITAL LOW\tP{:02}", pin),
        }
    }
}

/// An input to perform at a specific time.
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Operation {
    /// Time to perform the input in milliseconds
    pub time: u64,
    /// Signal to apply
    pub input: Signal,
}

impl Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}\tinput: {}", self.time, self.input)
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
    output: Signal,
}

impl Response {
    pub fn new(time: Instant, output: Signal) -> Response {
        Response {
            time,
            output,
        }
    }

    pub fn get_offset(&self, t0: Instant) -> Duration {
        self.time - t0
    }

    pub fn get_output(&self) -> &Signal {
        &self.output
    }
}

impl Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "output: {}", self.output)
    }
}

/** Defined response to look for from the device under test.

Criterion are used by [`Test`]s to determine how to inspect the output from a device under test.
 */
#[derive(Clone, Debug)]
pub enum Criterion {
    /// Record any response on the specified pin.
    Response(u8),
}

impl Display for Criterion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Criterion::Response(pin_no) => write!(f, "any activity on device pin {}", pin_no),
        }
    }
}

/// Test execution information
#[derive(Clone, Debug)]
pub struct Execution {
    started_at: Instant,
    finished_at: Instant,
}

impl Execution {
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
    pub fn get_duration(&self) -> Duration {
        self.finished_at - self.started_at
    }
}

/** Test definition.

A test mainly consists of a timeline of [`Operation`]s to perform (inputs to the device under test)
and a set of responses ([`Criterion`]) to record (outputs from the device under test).

Executing a test (via [`Test::execute`] produces an [`Execution`] that contains information about the test run.

 */
#[derive(Clone)]
pub struct Test {
    id: String,
    actions: BinaryHeap<Reverse<Operation>>,
    criteria: Vec<Criterion>,
}

impl Test {
    /// Define a new test.
    pub fn new<'a, T, U>(id: &str, ops: T, criteria: U) -> Test where
        T: IntoIterator<Item = &'a Operation>,
        U: IntoIterator<Item = &'a Criterion> {
        Test {
            id: id.to_string(),
            actions: ops.into_iter().map(|x| Reverse(*x)).collect(),
            criteria: criteria.into_iter().cloned().collect(),
        }
    }

    /// Returns the identifier of the test definition.
    pub fn get_id(&self) -> &str {
        &self.id
    }

    /// Returns defined criteria.
    pub fn get_criteria(&self) -> &Vec<Criterion> {
        &self.criteria
    }

    /// Drive test outputs (inputs to the device).
    pub fn execute(&self, t0: Instant, pins: &mut DeviceInputs) -> Result<Execution> {
        let timeline = self.actions.iter()
            .map(|Reverse(op)| (t0 + Duration::from_millis(op.time), op.input));
        for (t, input) in timeline {
            while Instant::now() < t {  } // spin wait
            match input {
                Signal::High(pin_no) =>
                    (*pins.get_pin_mut(pin_no)?)
                    .set_high(),
                Signal::Low(pin_no) =>
                    (*pins.get_pin_mut(pin_no)?)
                    .set_low(),
            };
            println!("{:?}", input);
        }

        Ok(Execution::new(t0, Instant::now()))
    }

    /// Set up to record test inputs.
    pub fn prep_observe(&self, pins: &mut DeviceOutputs) -> Result<()> {
        for criterion in &self.criteria {
            println!("observer: watching for {}", criterion);
            match criterion {
                Criterion::Response(pin_no) => {
                    pins.get_pin_mut(*pin_no)?
                        .set_interrupt(Trigger::Both)?;
                },
            };
        }

        Ok(())
    }

    /// Record test inputs (outputs from the device).
    pub fn observe(&self, t0: Instant, pins: &DeviceOutputs, out: &mut Vec<Response>) -> Result<()> {
        let gpio = Gpio::new()?;
        let t_end = t0 + self.get_max_runtime();
        let mut t = Instant::now();
        let pins = pins.get()?;

        while t < t_end {
            let poll = gpio.poll_interrupts(
                pins.as_slice(),
                false,
                Some(t_end - t))?;

            if let Some((pin, level)) = poll {
                let response = Response::new(
                    Instant::now(),
                    match level {
                        Level::High => Signal::High(pin.pin()),
                        Level::Low => Signal::Low(pin.pin()),
                    });
                out.push(response);
            }

            t = Instant::now();
        }

        Ok(())
    }

    /// Return the maximum length of time the test can run.
    ///
    /// TODO: make this dependent on actions' timing, criteria timing, and another tail duration(?).
    fn get_max_runtime(&self) -> Duration {
        let ms = self.actions.iter()
            .map(|Reverse(action)| action.time)
            .last()
            .unwrap_or(0)
            + 500;
        Duration::from_millis(ms)
    }
}

impl Display for Test {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Test: {}\n", self.id)?;
        write!(f, "=== Operation timeline\n")?;
        write!(f, "|{:>10}|{:^5}|{:^20}|\n", "time (ms)", "pin", "operation")?;
        write!(f, "|----------|-----|--------------------|\n")?;
        for Reverse(ref action) in &self.actions {
            let (sig_text, pin_no) = match action.input {
                Signal::High(p) => ("HIGH", p),
                Signal::Low(p) => ("LOW", p),
            };
            write!(f, "|{:>10}|{:^5}|{:^20}|\n", action.time, pin_no, sig_text)?;
        }
        write!(f, "\n")?;

        write!(f, "=== Criteria\n")?;
        for criterion in &self.criteria {
            write!(f, "- {}\n", criterion)?;
        }

        Ok(())
    }
}
