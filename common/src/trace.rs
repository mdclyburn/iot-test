//! Interpret execution trace information emitted from a DUT.

use std::fmt;
use std::fmt::Display;
use std::time::{Duration, Instant};

/// Purpose of a tracing channel.
pub enum TraceKind {
    /// Tracking processed by a means external to Clockwise.
    Raw,
    /// Control flow tracking.
    ControlFlow,
    /// Memory usage tracking.
    Memory,
    /// Benchmarking and data flow tracking.
    Performance(BenchmarkMetadata),
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

/// Information to interpret a waypoint.
#[derive(Clone)]
pub struct WaypointMetadata {
    /// An identifying name of the waypoint.
    pub label: String,
    /// Unit of the data measurement.
    pub unit: String,
}

const MAX_WAYPOINT_LABELS: usize = 8;

/// Information to interpret performance tracking data.
pub struct BenchmarkMetadata {
    waypoints: [Option<WaypointMetadata>; MAX_WAYPOINT_LABELS],
}

impl BenchmarkMetadata {
    /// Create a new `BenchmarkMetadata`.
    pub fn new(waypoints: &[WaypointMetadata]) -> BenchmarkMetadata {
        let waypoints = {
            let waypoints_iter = waypoints.iter();
            let mut waypoints_dest = [None, None, None, None,
                                      None, None, None, None];

            for (wp_dst, wp_src) in waypoints_dest.iter_mut().zip(waypoints_iter) {
                *wp_dst = Some(wp_src.clone());
            }

            waypoints_dest
        };

        BenchmarkMetadata {
            waypoints,
        }
    }
}
