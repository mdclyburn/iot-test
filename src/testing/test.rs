//! Defining and executing tests

use std::cmp::{Ord, Ordering, PartialOrd, Reverse};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fmt;
use std::fmt::Display;
use std::iter::IntoIterator;
use std::time::{Duration, Instant};

use rppal::gpio::{Gpio, Level, Trigger};

use crate::comm::Signal;
use crate::facility::EnergyMetering;
use crate::io::{DeviceInputs, DeviceOutputs};
use crate::sw::Platform;

use super::Error;

type Result<T> = std::result::Result<T, Error>;

/// An input to perform at a specific time.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Operation {
    /// Time to perform the input in milliseconds
    pub time: u64,
    /// Signal to apply
    pub input: Signal,
    /// Device pin to apply the signal to.
    pub pin_no: u8,
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

/// An output response from the device under test.
#[derive(Copy, Clone, Debug)]
pub struct Response {
    time: Instant,
    pin_no: u8,
    output: Signal,
}

impl Response {
    /// Create a new Response.
    pub fn new(time: Instant, pin_no: u8, output: Signal) -> Response {
        Response {
            time,
            pin_no,
            output,
        }
    }

    /** Obtain the amount of time between a fixed point and the occurence of this Response.

    This is typically used to get the point in time during a test a response occured.

    # Panics
    This function performs arithmetic with [`Instant`]s which will panic if `t0` is after the time this Response occured.
     */
    pub fn get_offset(&self, t0: Instant) -> Duration {
        self.time - t0
    }

    pub fn remapped(&self, host_target_map: &HashMap<u8, u8>) -> Response {
        let target_pin = host_target_map.get(&self.pin_no)
            .expect("Cannot remap device response because pin mapping does not exist.");

        Response {
            pin_no: *target_pin,
            .. *self
        }
    }
}

impl Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "response on P{:02} {}", self.pin_no, self.output)
    }
}

/** Defined response to look for from the device under test.

Criterion are used by [`Test`]s to determine how to inspect the output from a device under test.
 */
#[allow(unused)]
#[derive(Clone, Debug)]
pub enum Criterion {
    /// GPIO activity.
    GPIO(GPIOCriterion),
    /// Energy consumption.
    Energy(EnergyCriterion),
}

impl Display for Criterion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Criterion::GPIO(ref c) => write!(f, "GPIO activity: {}", c),
            Criterion::Energy(ref c) => write!(f, "Energy: {}", c),
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

/// Test execution information
#[derive(Clone, Debug)]
pub struct Execution {
    started_at: Instant,
    finished_at: Instant,
}

impl Execution {
    /// Create a new Execution.
    fn new(started_at: Instant, finished_at: Instant) -> Execution {
        Execution {
            started_at,
            finished_at,
        }
    }

    /// Return the point in time the test execution started.
    pub fn get_start(&self) -> &Instant {
        &self.started_at
    }

    /// Return the length of time the test ran for.
    pub fn duration(&self) -> Duration {
        self.finished_at - self.started_at
    }
}

/** Test definition.

A test mainly consists of a timeline of [`Operation`]s to perform (inputs to the device under test)
and a set of responses ([`Criterion`]) to record (outputs from the device under test).

Executing a test (via [`Test::execute`]) produces an [`Execution`] that contains information about the test run.

 */
#[derive(Clone, Debug)]
pub struct Test {
    id: String,
    platform: Platform,
    app_ids: HashSet<String>,
    trace_points: HashSet<String>,
    actions: BinaryHeap<Reverse<Operation>>,
    criteria: Vec<Criterion>,
    tail_duration: Option<Duration>,
}

impl Test {
    /// Define a new test.
    pub fn new<'a, T, U, V, W>(id: &str,
                               app_id: T,
                               trace_points: U,
                               ops: V,
                               criteria: W) -> Test
    where
        T: IntoIterator<Item = &'a str>,
        U: IntoIterator<Item = &'a str>,
        V: IntoIterator<Item = &'a Operation>,
        W: IntoIterator<Item = &'a Criterion>,
    {
        Test {
            id: id.to_string(),
            platform: Platform::Tock, // TODO: support different platforms
            app_ids: app_id.into_iter().map(|id| id.to_string()).collect(),
            trace_points: trace_points.into_iter().map(|tp| tp.to_string()).collect(),
            actions: ops.into_iter().map(|x| Reverse(*x)).collect(),
            criteria: criteria.into_iter().cloned().collect(),
            tail_duration: Some(Duration::from_millis(5)),
        }
    }

    /// Returns the identifier of the test definition.
    pub fn get_id(&self) -> &str {
        &self.id
    }

    /// Returns the platform the test is for.
    pub fn get_platform(&self) -> Platform {
        self.platform
    }

    /// Returns the identifiers of the applications the test exercises.
    pub fn get_app_ids(&self) -> &HashSet<String> {
        &self.app_ids
    }

    /// Returns the trace points the test requires.
    pub fn get_trace_points(&self) -> &HashSet<String> {
        &self.trace_points
    }

    /// Returns defined test criteria.
    pub fn get_criteria(&self) -> &Vec<Criterion> {
        &self.criteria
    }

    /// Drive test outputs (inputs to the device).
    pub fn execute(&self, t0: Instant, pins: &mut DeviceInputs) -> Result<Execution> {
        let timeline = self.actions.iter()
            .map(|Reverse(op)| (t0 + Duration::from_millis(op.time), op));
        for (t, op) in timeline {
            while Instant::now() < t {  } // spin wait?
            match op.input {
                Signal::Digital(true) =>
                    (*pins.get_pin_mut(op.pin_no)?)
                    .set_high(),
                Signal::Digital(false) =>
                    (*pins.get_pin_mut(op.pin_no)?)
                    .set_low(),
                input => panic!("Unhandled input type: {:?}", input),
            };
        }

        Ok(Execution::new(t0, Instant::now()))
    }

    /// Set up to record test inputs.
    pub fn prep_observe(&self, pins: &mut DeviceOutputs) -> Result<()> {
        let gpio_criteria = self.criteria.iter()
            .filter_map(|criterion| {
                if let Criterion::GPIO(gpio_crit) = criterion {
                    Some(gpio_crit)
                } else {
                    None
                }
            });
        for criterion in gpio_criteria {
            println!("observer: watching for {}", criterion);
            match criterion {
                GPIOCriterion::Any(pin_no) => {
                    pins.get_pin_mut(*pin_no)?
                        .set_interrupt(Trigger::Both)?;
                },
            };
        }

        Ok(())
    }

    /// Record test inputs (outputs from the device).
    ///
    /// Watches for responses from the device under test for a slightly longer duration than the duration of the test.
    /// This is done to catch any straggling responses from the device.
    pub fn observe(&self, t0: Instant, pins: &DeviceOutputs, out: &mut Vec<Response>) -> Result<()> {
        let gpio = Gpio::new()?;
        let t_end = t0 + self.get_max_runtime();
        let mut t = Instant::now();
        let pins = pins.get()?;

        while t < t_end {
            let poll = gpio.poll_interrupts(
                pins.as_slice(),
                false,
                Some(t_end - t))?;

            if let Some((pin, level)) = poll {
                let response = Response::new(
                    Instant::now(),
                    pin.pin(),
                    match level {
                        Level::High => Signal::Digital(true),
                        Level::Low => Signal::Digital(false),
                    });
                out.push(response);
            }

            t = Instant::now();
        }

        Ok(())
    }

    /// Prepare structures for energy metering.
    ///
    /// # Returns
    /// Returns true if there are energy metering criteria in this test.
    /// [`Test::meter`] should be called when running the test.
    pub fn prep_meter(&self,
                      meters: &HashMap<String, Box<dyn EnergyMetering>>,
                      out: &mut HashMap<String, Vec<f32>>,
    ) -> Result<bool> {
        // only care about meters defined in the criteria
        out.clear();

        let approx_loop_micros = 545;
        let max_sample_count = (self.get_max_runtime().as_micros() /
                                approx_loop_micros as u128) + 1;

        let mut has_energy_criteria = false;
        // pre-allocate space in sample output vectors
        for criterion in &self.criteria {
            if let Criterion::Energy(ref energy_criterion) = criterion {
                has_energy_criteria = true;
                let meter_id = energy_criterion.get_meter();
                if !meters.contains_key(meter_id) {
                    return Err(Error::NoSuchMeter(meter_id.to_string()));
                } else {
                    out.entry(meter_id.to_string())
                        .or_insert(Vec::new())
                        .reserve_exact(max_sample_count as usize);
                }
            }
        }

        Ok(has_energy_criteria)
    }

    /// Perform energy metering.
    ///
    /// The `out` parameter should be the same `out` passed to [`Test::prep_meter`].
    pub fn meter(&self, meters: &HashMap<String, Box<dyn EnergyMetering>>, out: &mut HashMap<String, Vec<f32>>) {
        let start = Instant::now();
        let runtime = self.get_max_runtime();

        // Without the call to thread::sleep, a single loop iteration
        // takes between 545.568us and 699.682us, averages 568.521us.
        // Reading a single meter in this loop yields me 94 samples.
        // So, sampling interval is actually: self.energy_sampling_rate + ~.5ms.
        // It makes sense to lose about 5 out of 100 samples for
        // self.energy_sampling_rate = 10ms given a test that executes for
        // 1000ms. 1000ms / 10.5ms/samples = 95.238 samples.
        loop {
            if Instant::now() - start >= runtime { break; }

            for (id, buf) in &mut *out {
                let meter = meters.get(id).unwrap();
                buf.push(meter.power());
            }
        }
    }

    /// Return the maximum length of time the test can run.
    ///
    /// TODO: make this dependent on actions' timing, criteria timing, and another tail duration(?).
    fn get_max_runtime(&self) -> Duration {
        let duration_ms = self.actions.iter()
            .map(|Reverse(action)| action.time)
            .last()
            .unwrap_or(0);
        let tail_ms = self.tail_duration
            .unwrap_or(Duration::from_millis(0))
            .as_millis();

        Duration::from_millis(duration_ms + tail_ms as u64)
    }
}

impl Display for Test {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Test: {}\n", self.id)?;
        write!(f, "=== Operation timeline\n")?;
        write!(f, "|{:>10}|{:^5}|{:^20}|\n", "time (ms)", "pin", "operation")?;
        write!(f, "|----------+-----+--------------------|\n")?;
        for Reverse(ref action) in &self.actions {
            let sig_text = match action.input {
                Signal::Digital(true) => "digital 1".to_string(),
                Signal::Digital(false) => "digital 0".to_string(),
                Signal::Analog(lv) => format!("analog {:5}", lv),
            };
            write!(f, "|{:>10}|{:^5}|{:^20}|\n",
                   action.time,
                   action.pin_no,
                   sig_text)?;
        }
        write!(f, "\n")?;

        write!(f, "=== Criteria\n")?;
        for criterion in &self.criteria {
            write!(f, "- {}\n", criterion)?;
        }

        Ok(())
    }
}
