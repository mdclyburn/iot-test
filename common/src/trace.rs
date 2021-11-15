//! Interpret GPIO-based execution trace information.

use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::time::{Duration, Instant};

use crate::comm::Signal;
use crate::sw::instrument::Spec;

use super::test::Response;

/// Trace execution information derived from GPIO activity.
#[derive(Clone, Debug)]
pub struct ParallelTrace {
    id: u16,
    extra: u16,
    responses: Vec<Response>,
}

impl ParallelTrace {
    /// Construct a new Trace.
    fn new(id: u16, extra: u16, responses: Vec<Response>) -> ParallelTrace {
        ParallelTrace {
            id,
            extra,
            responses,
        }
    }

    /// Returns the ID of the trace.
    pub fn get_id(&self) -> u16 {
        self.id
    }

    /// Returns data transmitted by extra data pins for the trace.
    pub fn get_extra(&self) -> u16 {
        self.extra
    }

    /** Returns the time the trace point was triggered.

    This is equivalent to the time the first pin in the set of GPIO trace pins was set by the hardware under test.
     */
    pub fn get_time(&self) -> Instant {
        self.responses[0].get_time()
    }

    /** Returns the length of time between the triggering of this Trace and the provided Instant.

    If the `t0` occurs before the Trace's triggering time, this function returns a 0-length Duration.
     */
    pub fn get_offset(&self, t0: Instant) -> Duration {
        if t0 < self.get_time() {
            self.get_time() - t0
        } else {
            Duration::from_millis(0)
        }
    }
}

impl Display for ParallelTrace {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Trace - ID: {}, data: {}\nRaw responses:\n", self.id, self.extra)?;
        for r in &self.responses {
            write!(f, "  {}\n", r)?;
        }

        Ok(())
    }
}

/// Derive [`ParallelTrace`]s from the provided GPIO activity.
pub fn reconstruct_parallel<'a, T>(responses: T,
                                   test_spec: &Spec,
                                   pin_sig: &HashMap<u8, u16>) -> Vec<ParallelTrace>
where
    T: IntoIterator<Item = &'a Response>
{
    if pin_sig.len() == 0 { return Vec::new(); }

    let last_trace_pin = *pin_sig.iter()
        .reduce(|(pin_no_a, sig_a), (pin_no_b, sig_b)| {
            if sig_a > sig_b {
                (pin_no_a, sig_a)
            } else {
                (pin_no_b, sig_b)
            }
        })
        .unwrap()
        .0;

    let mut traces = Vec::new();
    let mut response_iter = responses.into_iter();
    loop {
        let mut trace_responses: Vec<Response> = Vec::new();
        while let Some(response) = response_iter.next() {
            trace_responses.push(*response);
            if response.get_pin() == last_trace_pin {
                break;
            }
        }
        if trace_responses.is_empty() {
            break;
        }

        // Create Trace from pin responses.
        let mut trace_val: u16 = 0;
        for response in &trace_responses {
            if response.get_output() == Signal::Digital(true) {
                trace_val |= 1 << pin_sig.get(&response.get_pin()).unwrap();
            }
        }

        let trace = ParallelTrace::new(
            trace_val & id_mask(test_spec.id_bit_length()),
            (trace_val & extra_mask(test_spec.id_bit_length())) >> test_spec.id_bit_length(),
            trace_responses);

        traces.push(trace);
    }

    traces
}

/// Returns the mask of a given length for the ID bits.
fn id_mask(len: u8) -> u16 {
    let mut mask = 0;
    for n in 0..len {
        mask |= 1 << n;
    }

    mask
}

/// Returns the mask of a given length for the extra data bits.
fn extra_mask(id_len: u8) -> u16 {
    u16::MAX ^ id_mask(id_len)
}

/// Trace execution information derived from UART communication.
#[derive(Clone, Debug)]
pub struct SerialTrace {
    time: Instant,
    raw_data: Vec<u8>,
}

impl SerialTrace {
    /// Create a new serial trace.
    pub fn new(time: Instant, raw_data: &[u8]) -> SerialTrace {
        SerialTrace {
            time,
            raw_data: Vec::from(raw_data),
        }
    }

    /// Returns the size of the trace data.
    pub fn len(&self) -> usize {
        self.raw_data.len()
    }

    /// Returns the raw trace data.
    pub fn get_data(&self) -> &[u8] {
        self.raw_data.as_slice()
    }

    /// Returns the time the trace arrived.
    pub fn get_time(&self) -> Instant {
        self.time
    }

    /// Calculates the offset from the given time to the time the trace arrived.
    ///
    /// If `t0` is less than the Instant the trace arrived, this function returns an empty Duration.
    pub fn get_offset(&self, t0: Instant) -> Duration {
        if t0 < self.time {
            self.time - t0
        } else {
            Duration::from_millis(0)
        }
    }
}

impl Display for SerialTrace {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[ ")?;
        for byte in &self.raw_data {
            write!(f, "{:#02X} ", byte)?;
        }
        write!(f, "]")?;

        Ok(())
    }
}

/// Create structured [`SerialTrace`]s from raw UART data.
pub fn reconstruct_serial<'a, T>(raw_data: &[u8], timings: T) -> Vec<SerialTrace>
where
    T: IntoIterator<Item = &'a (Instant, usize)>
{
    let mut traces: Vec<SerialTrace> = Vec::new();
    let mut byte_no = 0;
    let timings = timings.into_iter().copied();

    for (t_recv, no_bytes) in timings {
        let trace = SerialTrace::new(t_recv, &raw_data[byte_no..byte_no+no_bytes]);
        traces.push(trace);
        byte_no += no_bytes;
    }

    traces
}
