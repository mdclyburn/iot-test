//! Evaluation specification criteria.

use std::fmt;
use std::fmt::Display;
use std::time::Duration;

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

#[derive(Copy, Clone, Debug)]
pub struct Trace {
    id: u8,
    extra: Option<u8>,
    time: Option<Duration>,
    time_tolerance: Option<Duration>,
}

impl Trace {
    pub fn new(id: u8) -> Trace {
        Trace {
            id,
            extra: None,
            time: None,
            time_tolerance: None,
        }
    }

    pub fn get_id(&self) -> u8 {
        self.id
    }

    pub fn get_extra_data(&self) -> Option<u8> {
        self.extra
    }

    pub fn get_time(&self) -> Option<Duration> {
        self.time
    }

    pub fn with_extra_data(self, extra: u8) -> Self {
        Self {
            extra: Some(extra),
            ..self
        }
    }

    pub fn with_timing(self, time: Duration, tolerance: Option<Duration>) -> Self {
        Self {
            time: Some(time),
            time_tolerance: tolerance,
            ..self
        }
    }
}

#[derive(Clone, Debug)]
pub struct TraceCriterion {
    occurrences: Vec<Trace>,
}

impl TraceCriterion {
    pub fn new<T>(traces: T) -> TraceCriterion
    where
        T: IntoIterator<Item = Trace>
    {
        TraceCriterion {
            occurrences: traces.into_iter().collect(),
        }
    }
}

impl Display for TraceCriterion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TRACE CRITERION (Display unimplemented)")
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
