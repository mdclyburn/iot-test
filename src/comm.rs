//! Definitions for communications-related primitives.

use std::fmt;
use std::fmt::Display;

/// Direction of information flow.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Direction {
    /// Input in the given context
    In,
    /// Output in the given context
    Out,
}

/// Signal class.
#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub enum Class {
    /// Analog signal
    Analog,
    /// Digital signal
    Digital,
}

/// A signal value.
#[allow(dead_code)]
#[derive(Copy, Clone, Eq, Debug, PartialEq)]
pub enum Signal {
    /// Digital; true for high, false for low
    Digital(bool),
    /// Analog
    Analog(u32),
}

impl Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Signal::Digital(lv) => write!(f, "Digital {}", if lv { "1" } else { "0" }),
            Signal::Analog(lv) => write!(f, "Analog {:5}", lv),
        }
    }
}
