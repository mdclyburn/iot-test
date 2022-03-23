//! Interpret execution trace information emitted from a DUT.

use std::convert::From;
use std::fmt;
use std::fmt::Display;
use std::time::{Duration, Instant};

use rppal::uart;
use rppal::uart::Uart;

use crate::io;
use crate::io::{IOError, UART};

type Result<T> = std::result::Result<T, String>;

/// Purpose of a tracing channel.
#[derive(Clone, Debug)]
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

impl TraceKind {
    /// Returns a short name suitable for labelling.
    pub fn label(&self) -> &'static str {
        use TraceKind::*;
        match self {
            Raw => "raw",
            ControlFlow => "control-flow",
            Memory => "memory",
            Performance(ref _meta) => "performance",
        }
    }
}

impl Display for TraceKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use TraceKind::*;
        match self {
            Raw => write!(f, "raw trace data"),
            ControlFlow => write!(f, "control flow trace data"),
            Memory => write!(f, "memory usage trace data"),
            Performance(ref _meta) => write!(f, "performance benchmarking data"),
        }
    }
}

/// Trace data organized by type of the data.
#[derive(Debug)]
pub enum TraceData {
    /// Raw tracing data, given as a sequence of bytes.
    Raw(Vec<u8>),
    /// Control flow tracing data.
    ControlFlow(Vec<SerialTrace>),
    /// Memory usage data.
    Memory(Vec<SerialTrace>),
    /// Performance benchmarking data.
    Performance(Vec<PeriodMetric>),
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
#[derive(Clone, Debug)]
pub struct WaypointMetadata {
    /// An identifying name of the waypoint.
    pub label: String,
    /// Unit of the data measurement.
    pub unit: String,
}

const MAX_WAYPOINT_LABELS: usize = 8;

/// Information to interpret performance tracking data.
#[derive(Clone, Debug)]
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

    /// Return metadata about the specified waypoint.
    fn waypoint_no(&self, no: usize) -> &WaypointMetadata {
        self.waypoints[no].as_ref().unwrap()
    }
}

/// Measurements from performance benchmarking passing the same data.
#[derive(Debug)]
pub struct PeriodMetric {
    t_start: f32,
    t_ends: [f32; MAX_WAYPOINT_LABELS],
    data_size: u32,
}

impl PeriodMetric {
    /// Create a new metric.
    pub fn new<T>(t_start: f32, data_size: u32, waypoint_t_ends: T) -> PeriodMetric
    where
        T: IntoIterator<Item = f32>,
    {
        let mut t_ends: [f32; MAX_WAYPOINT_LABELS] = [0.0; MAX_WAYPOINT_LABELS];
        for (src, dst) in waypoint_t_ends.into_iter().zip(&mut t_ends) {
            *dst = src;
        }

        PeriodMetric {
            t_start,
            t_ends,
            data_size,
        }
    }

    /// Returns the start time the metric accounts.
    pub fn start_time(&self) -> f32 {
        self.t_start
    }

    /// Returns the end time for a waypoint.
    pub fn end_time(&self, waypoint_no: usize) -> f32 {
        self.t_ends[waypoint_no]
    }

    /// Returns the total value of data counted in this instance.
    pub fn data_size(&self) -> u32 {
        self.data_size
    }
}

mod parsing {
    use nom::bytes::complete as bytes;
    use nom::{combinator, multi, sequence};

    use crate::parsing_support::{
        ByteError,
        ByteResult,
        little_u32,
        little_u64,
    };

    use super::PeriodMetric;

    /// Initialization data parser.
    fn benchmark_init<'a>(data: &'a [u8]) -> ByteResult<'a, u32> {
        sequence::preceded::<_, _, _, ByteError<'a>, _, _>(
            bytes::tag([0]),
            little_u32)
            (data)
    }

    /// Period metrics parser.
    fn benchmark_period_metrics<'a>(counter_freq: u32, data: &'a [u8]) -> ByteResult<'a, Vec<PeriodMetric>> {
        // We perform two separate parses here since there does not seem to be a method
        // for passing result and data from one parser to another.

        let mut parse_header = combinator::opt(sequence::preceded(
            // Header tag for the benchmarking data.
            bytes::tag([0b1000_0000]),
            // pair: <start time> <counter buckets>
            little_u64));

        match parse_header(data)? {
            (data, None) => Ok((data, Vec::new())),
            (data, Some(t_start)) => multi::many1(combinator::map(
                // The number of waypoints is not fixed but will be at least one.
                // Each is a pair of the 64-bit counter value and the data size accounted in the waypoint.
                multi::many1(sequence::pair(
                    combinator::map(little_u64, |cv: u64| (cv as f32) / (counter_freq as f32)),
                    little_u32)),
                move |wp_data: Vec<(f32, u32)>| {
                    PeriodMetric::new(
                        (t_start as f32) / (counter_freq as f32),
                        // Just take the first size for now.
                        // Later on, this may vary from waypoint to waypoint.
                        wp_data[0].1,
                        wp_data.iter().map(|(t_end, _ds)| *t_end))
                }))(data)
        }
    }

    /// Benchmark data complete parser.
    pub fn benchmark_data<'a>(data: &'a [u8]) -> ByteResult<Vec<PeriodMetric>> {
        let (data, freq) = benchmark_init(data)?;
        benchmark_period_metrics(freq, data)
    }
}

/// Size of a pre-allocated buffer.
const SERIAL_BUFFER_SIZE: usize = 64 * 1024;

/// Container structure for a buffer prepared to collect trace data.
///
/// [`prepare()`] provides a `PreparedBuffer` which can then be an argument to [`collect()`].
/// This ensures that `prepare()` has executed prior to the call to `collect()`.
pub struct PreparedBuffer<'a>(&'a mut Vec<u8>);

/// Prepare the a buffer and the UART for serial data collection.
pub fn prepare<'a>(buffer: &'a mut Vec<u8>, uart: &mut Uart) -> io::Result<PreparedBuffer<'a>> {
    buffer.clear();
    // Just use a constant size for now.
    // We have to push data into the buffer to make it possible to
    // update the values in place when the buffer is used as a slice.
    buffer.reserve(SERIAL_BUFFER_SIZE);
    while buffer.len() < SERIAL_BUFFER_SIZE { buffer.push(0); }

    uart.set_read_mode(0, Duration::from_millis(100))?;
    uart.flush(uart::Queue::Input)?;

    Ok(PreparedBuffer(buffer))
}

/// Collect tracing data from the given UART.
pub fn collect(kind: &TraceKind, uart: &mut Uart, buffer: PreparedBuffer, until: Instant) -> Result<TraceData> {
    // Collecting data for each trace kind is the same.
    // We are just reading bytes from the chosen serial line.

    // Shadow the buffer as a &mut [_].
    let buffer: &mut [u8] = buffer.0.as_mut_slice();
    let mut bytes_read: usize = 0;

    while Instant::now() < until {
        let read = uart.read(&mut buffer[bytes_read..])
            .unwrap();
        bytes_read += read;
    }
    println!("tracing read {} bytes", bytes_read);

    // Parsing the raw serial data is what is different.
    // Use the respective parser to recreate the structured data.
    match kind {
        TraceKind::Performance(ref _metadata) => parsing::benchmark_data(&buffer[0..bytes_read])
            .map(|(unparsed, metrics)| TraceData::Performance(metrics))
            .map_err(|e| format!("parsing error: {:?}", e)),

        _ => unimplemented!()
    }
}
