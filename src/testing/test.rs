use std::cmp::{Ord, Ordering, PartialOrd, Reverse};
use std::collections::BinaryHeap;
use std::convert::From;
use std::error;
use std::fmt;
use std::fmt::Display;
use std::iter::IntoIterator;
use std::time::{Duration, Instant};

use rppal::gpio::OutputPin;

use crate::io;
use crate::io::DeviceInputs;
use super::Error;

type Result<T> = std::result::Result<T, Error>;

#[derive(Copy, Clone, Eq, Debug, PartialEq)]
pub enum Signal {
    High(u8),
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

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Operation {
    pub time: u64,
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

#[derive(Clone, Debug)]
pub enum Criterion {
    Response(u8),
}

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

    pub fn get_duration(&self) -> &Duration {
        &self.duration
    }
}

#[derive(Clone)]
pub struct Test {
    id: String,
    actions: BinaryHeap<Reverse<Operation>>,
    criteria: Vec<Criterion>,
}

impl Test {
    pub fn new<'a, T, U>(id: &str, ops: T, criteria: U) -> Test where
        T: IntoIterator<Item = &'a Operation>,
        U: IntoIterator<Item = &'a Criterion> {
        Test {
            id: id.to_string(),
            actions: ops.into_iter().map(|x| Reverse(*x)).collect(),
            criteria: criteria.into_iter().cloned().collect(),
        }
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_criteria(&self) -> &Vec<Criterion> {
        &self.criteria
    }

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
