//! Interpret execution trace information emitted from a DUT.

use std::convert::From;
use std::fmt;
use std::time::{Duration, Instant};

use rppal::uart;
use rppal::uart::Uart;

use crate::io;

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

impl fmt::Display for TraceKind {
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
    Performance(PerformanceData),
}

impl TraceData {
    /// Create a `Display`able summary of trace information.
    pub fn summary<'a>(&'a self, info: &'a TraceKind) -> Display<'a> {
        Display {
            info,
            data: &self,
        }
    }
}

/// Helper struct for displaying a summary of tracing information.
pub struct Display<'a> {
    info: &'a TraceKind,
    data: &'a TraceData,
}

impl<'a> Display<'a> {
    fn panic_mismatch() -> ! {
        panic!("TraceKind-TraceData mismatch; this is a bug.");
    }

    fn display_performance(
        metadata: &BenchmarkMetadata,
        data: &PerformanceData,
        f: &mut fmt::Formatter) -> fmt::Result
    {
        let no_waypoints = data.no_waypoints as usize;

        for period in &data.metrics {
            // Show redundant metadata.
            write!(f, "Start: {:2.06}s\n", period.start_time())?;
            write!(f, "Data size: {} {}\n", period.data_size(), metadata.unit())?;

            // Headers
            let rate_text = format!("rate ({}/s)", metadata.unit());
            write!(f, "|   waypoint   |   t_end (s)   | duration (s) | {:^20} |\n", rate_text)?;
            // A row for each datapoint.
            for i in 0..no_waypoints {
                let duration: f64 = period.end_time(i) - period.start_time();
                let data_rate: f64 = (period.data_size() as f64) / duration;

                write!(f, "| {:^12} | {:13.06} | {:12.06} | {:20.06} |\n",
                       metadata.waypoint_no(i).as_ref().map_or("???", |w| &w.label),
                       period.end_time(i),
                       duration,
                       data_rate)?;
            }
        }

        Ok(())
    }
}

impl<'a> fmt::Display for Display<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Ensure the TraceKind is the correct variant for this data.
        match self.info {
            TraceKind::Raw => match self.data {
                TraceData::Raw(_data) => unimplemented!(),
                _ => Display::panic_mismatch(),
            },

            TraceKind::ControlFlow => match self.data {
                TraceData::ControlFlow(_data) => unimplemented!(),
                _ => Display::panic_mismatch(),
            },

            TraceKind::Memory => match self.data {
                TraceData::Memory(_data) => unimplemented!(),
                _ => Display::panic_mismatch(),
            },

            TraceKind::Performance(info) => match self.data {
                TraceData::Performance(data) =>
                    Display::display_performance(info, data, f),
                _ => Display::panic_mismatch(),
            }
        }
    }
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

impl fmt::Display for SerialTrace {
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
}

const MAX_WAYPOINT_LABELS: usize = 8;

/// Information to interpret performance tracking data.
#[derive(Clone, Debug)]
pub struct BenchmarkMetadata {
    unit: String,
    waypoints: [Option<WaypointMetadata>; MAX_WAYPOINT_LABELS],
}

impl BenchmarkMetadata {
    /// Create a new `BenchmarkMetadata`.
    pub fn new(unit: &str, waypoints: &[WaypointMetadata]) -> BenchmarkMetadata {
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
            unit: unit.to_string(),
            waypoints,
        }
    }

    fn unit(&self) -> &str {
        &self.unit
    }

    /// Return metadata about the specified waypoint.
    fn waypoint_no(&self, no: usize) -> &Option<WaypointMetadata> {
        &self.waypoints[no]
    }
}

/// Collected performance data.
#[derive(Clone, Debug)]
pub struct PerformanceData {
    no_waypoints: u8,
    metrics: Vec<PeriodMetric>,
}

impl PerformanceData {
    fn new<T>(no_waypoints: u8, metrics: T) -> PerformanceData
    where
        T: IntoIterator<Item = PeriodMetric>
    {
        PerformanceData {
            no_waypoints,
            metrics: metrics.into_iter().collect(),
        }
    }
}

/// Set of measurements for a set of points taken within the same length of time.
#[derive(Clone, Debug)]
pub struct PeriodMetric {
    t_start: f64,
    t_ends: [f64; MAX_WAYPOINT_LABELS],
    data_size: u32,
}

impl PeriodMetric {
    /// Create a new metric.
    pub fn new<T>(t_start: f64, data_size: u32, waypoint_t_ends: T) -> PeriodMetric
    where
        T: IntoIterator<Item = f64>,
    {
        let mut t_ends: [f64; MAX_WAYPOINT_LABELS] = [0.0; MAX_WAYPOINT_LABELS];
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
    pub fn start_time(&self) -> f64 {
        self.t_start
    }

    /// Returns the end time for a waypoint.
    pub fn end_time(&self, waypoint_no: usize) -> f64 {
        self.t_ends[waypoint_no]
    }

    /// Returns the total value of data counted in this instance.
    pub fn data_size(&self) -> u32 {
        self.data_size
    }
}

mod parsing {
    use nom::bits::complete as bits;
    use nom::bits::bits as adapt_bit_parser;
    use nom::bytes::complete as bytes;
    use nom::{combinator, multi, sequence};

    use crate::parsing_support::{
        BitError,
        ByteError,
        ByteResult,
        little_u32,
        little_u64,
    };

    use super::{PerformanceData, PeriodMetric};

    /// Initialization data parser.
    ///
    /// Returns a tuple: (no. of stat containers, counter frequency).
    fn benchmark_init<'a>(data: &'a [u8]) -> ByteResult<'a, (u8, u32)> {
        sequence::pair::<_, _, _, ByteError<'a>, _, _>(
            adapt_bit_parser::<_, _, BitError<'a>, _, _>(
                sequence::preceded(
                    bits::tag(0b0000, 4usize),
                    bits::take(4usize))),
            little_u32)
            (data)
    }

    /// Period metrics parser.
    fn benchmark_period_metrics<'a>(
        counter_freq: u32,
        no_containers: u8,
        data: &'a [u8]
    ) -> ByteResult<'a, PerformanceData>
    {
        // We perform two separate parses here since there does not seem to be a method
        // for passing result and data from one parser to another.

        combinator::map(
            multi::many0(combinator::map(
                // pair: <header> + <N stat containers>
                sequence::pair(
                    // preceded: <header tag> + <64-bit timestamp>
                    sequence::preceded(bytes::tag([0b1000_0000]), little_u64),
                    // count: exactly `no_containers` stat containers
                    multi::count(
                        // pair: <64-bit timestamp> + <32-bit accumulated data size>
                        sequence::pair(little_u64, little_u32),
                        no_containers as usize)),

                |(t_start, stats): (u64, Vec<(u64, u32)>)| {
                    PeriodMetric::new(
                        (t_start as f64) / (counter_freq as f64),
                        // Just take the first size for now, for simplicity's sake.
                        // Later on, this may vary from waypoint to waypoint.
                        stats[0].1,
                        stats.iter().map(|(t_end, _ds)| (*t_end as f64) / (counter_freq as f64)))
                })),
            |metrics| PerformanceData::new(no_containers, metrics))
            (data)
    }

    /// Benchmark data complete parser.
    pub fn benchmark_data<'a>(data: &'a [u8]) -> ByteResult<PerformanceData> {
        let (data, (no_stats, freq)) = benchmark_init(data)?;
        benchmark_period_metrics(freq, no_stats, data)
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
            .map(|(unparsed, data)| {
                println!("tracing left {} bytes unparsed", unparsed.len());
                TraceData::Performance(data)
            })
            .map_err(|e| format!("parsing error: {:?}", e)),

        _ => unimplemented!()
    }
}
