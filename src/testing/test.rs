//! Defining and executing tests

use std::cmp::{Ord, Ordering, PartialOrd, Reverse};
use std::collections::BinaryHeap;
use std::convert::From;
use std::error;
use std::fmt;
use std::fmt::Display;
use std::iter::IntoIterator;
use std::time::{Duration, Instant};

use crate::io;
use crate::io::DeviceInputs;
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
#[derive(Copy, Clone)]
pub struct Response {
    pub time: u64,
    pub output: Signal,
}

impl Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}\toutput: {}", self.time, self.output)
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

/// Test execution information
#[derive(Clone, Debug)]
pub struct Execution {
    duration: Duration,
}

impl Execution {
    fn new(duration: Duration) -> Execution {
        Execution {
            duration
        }
    }

    /// Return the length of time the test ran for (in milliseconds)
    pub fn get_duration(&self) -> &Duration {
        &self.duration
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

    /// Run the test.
    pub fn execute(&self, t0: Instant, pins: &DeviceInputs) -> Result<Execution> {
        let timeline = self.actions.iter()
            .map(|Reverse(op)| (t0 + Duration::from_millis(op.time), op.input));
        for (t, input) in timeline {
            while Instant::now() < t {  } // spin wait
            match input {
                Signal::High(pin_no) =>
                    (*pins.get_pin(pin_no)?)
                    .set_high(),
                Signal::Low(pin_no) =>
                    (*pins.get_pin(pin_no)?)
                    .set_low(),
            };
            println!("{:?}", input);
        }

        Ok(Execution::new(Instant::now() - t0))
    }
}

impl Display for Test {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Test: {}\n", self.id)?;
        write!(f, "Operations =====\n")?;
        for Reverse(ref action) in &self.actions {
            write!(f, "{}\n", action)?;
        }

        Ok(())
    }
}
