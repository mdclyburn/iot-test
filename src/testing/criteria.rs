//! Create and manipulate conditions to evaluate test executions against.

use std::cmp::Ord;
use std::fmt;
use std::fmt::Display;
use std::time::{Duration, Instant};

use super::trace::Trace;

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
    /// GPIO-based activity tracing.
    Trace(TraceCriterion),
}

impl Display for Criterion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Criterion::GPIO(ref c) => write!(f, "GPIO activity: {}", c),
            Criterion::Energy(ref c) => write!(f, "Energy: {}", c),
            Criterion::Trace(ref c) => write!(f, "Trace: {}", c),
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

/// Component condition of a [`TraceCriterion`].
#[derive(Copy, Clone, Debug)]
pub struct TraceCondition {
    id: u16,
    extra: Option<u16>,
    timing: Option<(Timing, Duration)>,
}

impl TraceCondition {
    /// Create a new tracing requirement.
    pub fn new(id: u16) -> TraceCondition {
        TraceCondition {
            id,
            extra: None,
            timing: None,
        }
    }

    /// Returns the ID that would satisfy the trace condition.
    #[allow(dead_code)]
    pub fn get_id(&self) -> u16 {
        self.id
    }

    /// Returns the extra data that would satisfy the trace condition.
    #[allow(dead_code)]
    pub fn get_extra_data(&self) -> Option<u16> {
        self.extra
    }

    /// If provided, returns the necessary time offset to satisfy the trace condition.
    #[allow(dead_code)]
    pub fn get_offset(&self) -> Option<Timing> {
        self.timing.as_ref()
            .map(|(timing, _tolerance)| *timing)
    }

    /// If provided, returns the timing tolerance to satisfy the trace condition.
    #[allow(dead_code)]
    pub fn get_tolerance(&self) -> Option<Duration> {
        self.timing.as_ref()
            .map(|(_timing, tolerance)| *tolerance)
    }

    /// Construct a trace condition with the specified extra data.
    ///
    /// This is a convenience function for test that may later be removed.
    #[allow(dead_code)]
    pub fn with_extra_data(self, extra: u16) -> Self {
        Self {
            extra: Some(extra),
            ..self
        }
    }

    /// Construct a trace condition with the specified timing.
    ///
    /// This is a convenience function for test that may later be removed.
    #[allow(dead_code)]
    pub fn with_timing(self, time: Timing, tolerance: Duration) -> Self {
        Self {
            timing: Some((time, tolerance)),
            ..self
        }
    }

    /// Returns true if the provided trace's ID and extra data satisfy the condition.
    ///
    /// Because the required timing of the condition is dependent on whether the timing is relative to the
    /// previous Trace or absolute (relative to the beginning of the test), this function does not check timing.
    fn satisfied_by(&self, event: &Trace) -> bool {
        event.get_id() == self.id
            && self.extra.map_or(true, |extra| event.get_extra() == extra)
    }
}

impl Display for TraceCondition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Trace point with ID {}", self.get_id())?;

        if let Some(extra) = self.get_extra_data() {
            write!(f, ", extra data {}", extra)?;
        }

        if let Some(timing) = self.get_offset() {
            write!(f, ", {} (tol: {:?})", timing, self.get_tolerance().unwrap())?;
        }

        Ok(())
    }
}

/// Trace criterion specification details.
#[derive(Clone, Debug)]
pub struct TraceCriterion {
    conditions: Vec<TraceCondition>,
}

impl TraceCriterion {
    /// Create a new trace criterion.
    pub fn new<'a, T>(conditions: T) -> TraceCriterion
    where
        T: IntoIterator<Item = &'a TraceCondition>
    {
        TraceCriterion {
            conditions: conditions.into_iter()
                .copied()
                .collect(),
        }
    }

    /// Returns the [`Trace`]s satisfying the criterion.
    pub fn align<'a>(&self, t0: Instant, traces: &'a [Trace]) -> Option<Vec<&'a Trace>> {
        TraceCriterion::rec_align(t0,
                                  t0,
                                  self.conditions.as_slice(),
                                  traces)
    }


    /** Attempt to satisfy conditions with the provided [`Trace`]s.

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
                     conditions: &[TraceCondition],
                     events: &'a [Trace]) -> Option<Vec<&'a Trace>>
    {
        let mut matches = Vec::new();

        if conditions.len() > 0 {
            let condition = conditions[0];
            for (event, idx) in events.iter().zip(0..) {
                // Check the timing of the trace event as that cannot be determined
                // within the context of the TraceCondition alone, especially if the
                // timing is relative to other conditions.
                if condition.satisfied_by(event) {
                    let timing_matches: bool = {
                        if let Some(timing) = condition.get_offset() {
                            // println!("Checking timing for trace event.");
                            // Calculate the time point test the trace condition
                            // specifies the trace should occur at.
                            let t_req = match timing {
                                Timing::Absolute(d) => t0 + d,
                                Timing::Relative(d) => tp + d,
                            };
                            // Difference between the actual event occurrence time and the specification's time point.
                            let since = t_req.max(event.get_time()) - t_req.min(event.get_time());
                            // println!("  req. offset: {:?}, tolerance: {:?}", since, condition.get_tolerance().unwrap());
                            // println!("  since time offset: {:?}", since);
                            since < condition.get_tolerance().unwrap()
                        } else {
                            true
                        }
                    };
                    // If the rest of the events in the condition chain are satisfied, then
                    // the criterion is satisfied. If not, we continue skimming over events.
                    if timing_matches {
                        let rest = TraceCriterion::rec_align(t0,
                                                             event.get_time(),
                                                             &conditions[1..],
                                                             &events[idx+1..]);
                        if let Some(rest) = rest {
                            matches.push(event);
                            matches.extend(rest.into_iter());
                            return Some(matches);
                        }
                    }
                }
            }

            // No more events to match. Game over.
            None
        } else {
            // No more conditions to try to match. We're finished.
            Some(Vec::new())
        }
    }
}

impl Display for TraceCriterion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for condition in &self.conditions {
            write!(f, "\n   → ID: {}, data: {}{}",
                   condition.get_id(),
                   condition.get_extra_data().map(|x| x.to_string()).unwrap_or("none".to_string()),
                   if let Some(timing) = condition.get_offset() {
                       format!(" @ {:?}±{:?} from {}",
                               timing.get_offset(),
                               condition.get_tolerance().unwrap(),
                               match timing {
                                   Timing::Absolute(_) => "test start",
                                   Timing::Relative(_) => "last event",
                               })
                   } else {
                       "".to_string()
                   })?;
        }

        Ok(())
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
