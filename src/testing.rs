use std::cmp::{Ord, Ordering, PartialOrd, Reverse};
use std::collections::BinaryHeap;
use std::fmt;
use std::fmt::Display;
use std::iter::IntoIterator;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
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

    pub fn execute(&self) -> Evaluation {
        // Run the test and produce an evaluation result.
        Evaluation::new("go-hon", Status::Invalid)
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

#[derive(Debug)]
pub struct Evaluation {
    test_id: String,
    outcome: Status,
}

impl Evaluation {
    pub fn new(test_id: &str, outcome: Status) -> Evaluation {
        Evaluation {
            test_id: test_id.to_string(),
            outcome: outcome,
        }
    }
}

impl Display for Evaluation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}\t{:?}", self.test_id, self.outcome)
    }
}

#[derive(Debug)]
pub enum Status {
    NotExecuted,
    Pass,
    Fail,
    Invalid,
}

impl From<Status> for &'static str {
    fn from(s: Status) -> Self {
        match s {
            Status::NotExecuted => "Not executed",
            Status::Pass => "Pass",
            Status::Fail => "Fail",
            Status::Invalid => "Invalid",
        }
    }
}
