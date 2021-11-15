//! Create and manipulate conditions to evaluate test executions against.

use std::cmp::Ord;
use std::fmt;
use std::fmt::Display;
use std::time::{Duration, Instant};

use super::trace::SerialTrace;

/** Defined response to look for from the device under test.

Criterion are used by [`super::test::Test`]s to determine how to inspect the output from a device under test.
 */
#[allow(unused)]
#[derive(Clone, Debug)]
pub enum Criterion {
    /// GPIO activity.
    GPIO(GPIOCriterion),
    /// Energy consumption.
    Energy(EnergyCriterion),
    /// Serial-based activity tracing.
    SerialTrace(SerialTraceCriterion),
}

impl Display for Criterion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Criterion::GPIO(ref c) => write!(f, "GPIO activity: {}", c),
            Criterion::Energy(ref c) => write!(f, "Energy: {}", c),
            Criterion::SerialTrace(ref c) => write!(f, "Serial trace: {}", c),
        }
    }
}

/// Trackable GPIO activity.
#[allow(unused)]
#[derive(Clone, Copy, Debug)]
pub enum GPIOCriterion {
    /// Any and all activity on a GPIO pin.
    Any(u8),
}

impl Display for GPIOCriterion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GPIOCriterion::Any(pin_no) => write!(f, "any output on device pin {}", pin_no),
        }
    }
}

/// Timing requirement.
#[derive(Copy, Clone, Debug)]
pub enum Timing {
    /// Point in time relative to the start of the test.
    Absolute(Duration),
    /// Point in time relative to the previous event.
    Relative(Duration),
}

impl Timing {
    /// Returns the contained offset of the Timing.
    fn get_offset(&self) -> Duration {
        match self {
            Timing::Absolute(d) => *d,
            Timing::Relative(d) => *d,
        }
    }
}

impl Display for Timing {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Timing::*;
        let ref_point = match self {
            Absolute(_) => "start of test",
            Relative(_) => "previous event",
        };
        write!(f, "{:?} from {}", self.get_offset(), ref_point)
    }
}

/// Energy criterion specification details.
#[derive(Clone, Debug)]
pub struct EnergyCriterion {
    meter: String,
    stat: EnergyStat,
    min: Option<f32>,
    max: Option<f32>,
}

/// Energy-specific criterion of interest.
impl EnergyCriterion {
    /// Create a new EnergyCriterion.
    #[allow(dead_code)]
    pub fn new(meter: &str, stat: EnergyStat) -> Self {
        Self {
            meter: meter.to_string(),
            stat,
            min: None,
            max: None,
        }
    }

    /// Specify a minimum value for the criterion.
    #[allow(unused)]
    pub fn with_min(self, min: f32) -> Self {
        Self {
            min: Some(min),
            ..self
        }
    }

    /// Specify a maximum value for the energy criterion.
    #[allow(unused)]
    pub fn with_max(self, max: f32) -> Self {
        Self {
            max: Some(max),
            ..self
        }
    }

    /// Returns the name of the target energy meter.
    pub fn get_meter(&self) -> &str {
        &self.meter
    }

    /// Returns the energy statistic.
    pub fn get_stat(&self) -> EnergyStat {
        self.stat
    }

    /** Returns true if the given value violates the criterion.

    If there is no part of the criterion can be violated this function will return None.
     */
    pub fn violated(&self, value: f32) -> Option<bool> {
        if self.min.is_none() && self.max.is_none() {
            None
        } else {
            let b = self.min.map(|min| value < min)
                .unwrap_or(false)
                ||
                self.max.map(|max| value > max)
                .unwrap_or(false);

            Some(b)
        }
    }
}

impl Display for EnergyCriterion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let unit = match self.stat {
            EnergyStat::Total => "mJ",
            _ => "mJ/s"
        };

        write!(f, "'{}' {} ", self.meter, self.stat)?;
        write!(f, "(min: {},", self.min.map(|x| format!("{:.2}{}", x, unit)).unwrap_or("-".to_string()))?;
        write!(f, " max: {})", self.max.map(|x| format!("{:.2}{}", x, unit)).unwrap_or("-".to_string()))?;

        Ok(())
    }
}

/// Trackable energy usage statistics.
#[allow(unused)]
#[derive(Clone, Copy, Debug)]
pub enum EnergyStat {
    /// Track total energy consumption.
    Total,
    /// Track average energy consumption rate.
    Average,
    /// Track the maximum energy consumption rate.
    Max,
    /// Track the minimum energy consumption rate.
    Min,
}

impl Display for EnergyStat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EnergyStat::Total => write!(f, "total consumption"),
            EnergyStat::Average => write!(f, "average consumption rate"),
            EnergyStat::Max => write!(f, "max consumption"),
            EnergyStat::Min => write!(f, "min consumption"),
        }
    }
}

/// Component condition of a [`SerialTraceCriterion`].
#[allow(unused)]
#[derive(Clone, Debug)]
pub struct SerialTraceCondition {
    data: Vec<u8>,
    timing: Option<(Timing, Duration)>,
}

impl SerialTraceCondition {
    /// Create a new serial tracing condition.
    pub fn new<'a, T>(data: T) -> SerialTraceCondition
    where
        T: IntoIterator<Item = &'a u8>,
    {
        SerialTraceCondition {
            data: data.into_iter()
                .copied()
                .collect(),
            timing: None,
        }
    }

    /// Specify the timing requirements to meet the condition.
    #[allow(dead_code)]
    pub fn with_timing(self, time: Timing, tolerance: Duration) -> Self {
        Self {
            timing: Some((time, tolerance)),
            ..self
        }
    }

    /// Returns the required data to match.
    pub fn get_data(&self) -> &Vec<u8> {
        &self.data
    }

    /// Returns the time requirement.
    pub fn get_offset(&self) -> Option<Timing> {
        self.timing.as_ref()
            .map(|(timing, _tolerance)| timing)
            .copied()
    }

    /// Returns the timing tolerance.
    pub fn get_tolerance(&self) -> Option<Duration> {
        self.timing.as_ref()
            .map(|(_timing, tolerance)| tolerance)
            .copied()
    }

    /// Returns true if the provided trace’s ID and extra data satisfy the condition.

    /// Because the required timing of the condition is dependent on whether the timing is relative
    /// to the previous Trace or absolute (relative to the beginning of the test), this function does not check timing.
    pub fn satisfied_by(&self, event: &SerialTrace) -> bool {
        self.data.len() == event.len()
            &&
            (&self.data).into_iter()
            .eq(event.get_data())
    }
}

/// Serial tracing criterion specification details.
#[allow(unused)]
#[derive(Clone, Debug)]
pub struct SerialTraceCriterion {
    conditions: Vec<SerialTraceCondition>,
}

impl SerialTraceCriterion {
    /// Create a new serial trace criterion.
    pub fn new<'a, T>(conditions: T) -> SerialTraceCriterion
    where
        T: IntoIterator<Item = &'a SerialTraceCondition>,
    {
        SerialTraceCriterion {
            conditions: conditions.into_iter()
                .cloned()
                .collect(),
        }
    }

    /// Returns the [`SerialTrace`]s satisfying the criterion.
    pub fn align<'a>(&self, t0: Instant, traces: &'a [SerialTrace]) -> Option<Vec<&'a SerialTrace>> {
        SerialTraceCriterion::rec_align(
            t0, t0, self.conditions.as_slice(), traces)
    }

    /** Attempt to satisfy conditions with the provided [`SerialTrace`]s.

    # Algorithm overview

    Advances through:
    - ordering of trace conditions
    - sequence of trace events captured during the test

    For each trace condition, advances through the trace events to find a matching trace event.
    Upon finding a matching trace condition, the function advances to the next trace condition.
    If a trace condition fails to find a matching trace event, then we back out to the previous trace condition.
    The previous trace condition seeks another matching trace event.
    If a trace condition advances to the last trace event and does not find a match, then the function returns false.
     */
    fn rec_align<'a>(t0: Instant,
                     tp: Instant,
                     conditions: &[SerialTraceCondition],
                     events: &'a [SerialTrace]) -> Option<Vec<&'a SerialTrace>>
    {
        let mut matches = Vec::new();

        if conditions.len() > 0 {
            let condition = &conditions[0];
            for (event, idx) in events.iter().zip(0..) {
                if condition.satisfied_by(event) {
                    let timing_matches: bool = {
                        if let Some(timing) = condition.get_offset() {
                            let t_req = match timing {
                                Timing::Absolute(d) => t0 + d,
                                Timing::Relative(d) => tp + d,
                            };

                            let since = t_req.max(event.get_time()) - t_req.min(event.get_time());
                            since < condition.get_tolerance().unwrap()
                        } else {
                            true
                        }
                    };

                    if timing_matches {
                        let rest = SerialTraceCriterion::rec_align(
                            t0, event.get_time(), &conditions[1..], &events[idx+1..]);
                        if let Some(rest) = rest {
                            matches.push(event);
                            matches.extend(rest.into_iter());

                            return Some(matches);
                        }
                    }
                }
            }

            None
        } else {
            Some(Vec::new())
        }
    }
}

impl Display for SerialTraceCriterion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for condition in &self.conditions {
            write!(f, "\n  → data: [ ")?;
            for byte in condition.get_data() {
                write!(f, "{:02X} ", byte)?;
            }
            write!(f, "]")?;

            if let Some(timing) = condition.get_offset() {
                write!(f, " @ {:?}±{:?} from {}",
                        timing.get_offset(),
                        condition.get_tolerance().unwrap(),
                        match timing {
                            Timing::Absolute(_) => "test start",
                            Timing::Relative(_) => "last event",
                        })?;
            }
        }

        Ok(())
    }
}
