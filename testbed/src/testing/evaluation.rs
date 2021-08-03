//! Insights into test executions.

use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::time::Duration;

use flexbed_common::criteria::{
    Criterion,
    GPIOCriterion,
    EnergyStat,
};
use flexbed_common::sw::instrument::Spec;
use flexbed_common::test::{
    Execution,
    Response,
    Test
};
use flexbed_common::trace::{
    ParallelTrace,
    SerialTrace,
};

use super::{Error, Result};

/// Summary of an `Evaluation`.
#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub enum Status {
    /// Execution finished without error.
    Complete,
    /// Execution completed and all criteria are satisfied.
    Pass,
    /// Execution completed, but one or more criteria are violated.
    Fail,
    /// Execution did not complete successfully.
    Error,
}

impl Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Status::Complete => write!(f, "Complete"),
            Status::Pass => write!(f, "Pass"),
            Status::Fail => write!(f, "Fail"),
            Status::Error => write!(f, "Error"),
        }
    }
}

/// In-depth information about a test execution.
#[derive(Debug)]
pub struct Evaluation {
    test: Test,
    spec: Option<Spec>,
    exec_result: Result<Execution>,
    device_responses: Vec<Response>,
    parallel_traces: Vec<ParallelTrace>,
    serial_traces: Vec<SerialTrace>,
    energy_metrics: HashMap<String, Vec<f32>>,
}

impl Evaluation {
    pub fn new(test: &Test,
               spec: &Spec,
               exec_result: Result<Execution>,
               device_responses: Vec<Response>,
               parallel_traces: Vec<ParallelTrace>,
               serial_traces: Vec<SerialTrace>,
               energy_metrics: HashMap<String, Vec<f32>>) -> Evaluation
    {
        Evaluation {
            test: test.clone(),
            spec: Some(spec.clone()),
            exec_result,
            device_responses,
            parallel_traces,
            serial_traces,
            energy_metrics,
        }
    }

    /// Create an evaluation that fails due to an error during testing.
    pub fn failed(test: &Test, spec: Option<&Spec>, error: Error) -> Evaluation {
        Evaluation {
            test: test.clone(),
            spec: spec.map(|s| s.clone()),
            exec_result: Err(error),
            device_responses: Vec::new(),
            parallel_traces: Vec::new(),
            serial_traces: Vec::new(),
            energy_metrics: HashMap::new(),
        }
    }

    /// Returns the execution result used in the evaluation.
    pub fn get_exec_result(&self) -> &Result<Execution> {
        &self.exec_result
    }

    /// Overall outcome of the evaluation.
    pub fn outcome(&self) -> Status {
        if self.exec_result.is_err() {
            Status::Error
        } else {
            Status::Complete
        }
    }

    // Come up with an evaluation for the given criterion.
    fn evaluate(&self, criterion: &Criterion) -> (Status, Option<String>) {
        match criterion {
            Criterion::GPIO(criterion) => {
                match criterion {
                    GPIOCriterion::Any(_pin) => (Status::Complete, None),
                }
            },

            Criterion::Energy(criterion) => {
                match criterion.get_stat() {
                    EnergyStat::Total => {
                        // Should exist in map because criterion stated it should be tracked.
                        let samples = self.energy_metrics.get(criterion.get_meter())
                            .unwrap();

                        let execution_duration = self.exec_result
                            .as_ref()
                            // Evaluation results are only relevant when the exec_result is Ok(...).
                            .expect("Attempted to evaluate criterion when execution result failed")
                            .duration();
                        let sample_count = samples.len();
                        // Approximate the time slice of each sample from the number of samples taken.
                        let sample_time_repr: Duration = execution_duration / sample_count as u32;
                        let rate_to_total_factor: f64 = sample_time_repr.as_micros() as f64
                            / Duration::from_secs(1).as_micros() as f64;

                        let mut total = 0f64;
                        for sample in samples.iter().copied() {
                            // mJ/s * fraction of the second the sample accounts for
                            total += sample as f64 * rate_to_total_factor;
                        }

                        let status = if let Some(violated) = criterion.violated(total as f32) {
                            if violated {
                                Status::Fail
                            } else {
                                Status::Pass
                            }
                        } else {
                            Status::Complete
                        };

                        (status, Some(format!("{:.2}mJ consumed", total)))
                    },

                    EnergyStat::Average => {
                        let samples = self.energy_metrics.get(criterion.get_meter()).unwrap();
                        // ASSUMPTION: timer intervals represented by all samples are equal.
                        let avg: f32 = samples.iter().sum::<f32>() / samples.len() as f32;

                        let status = if let Some(violated) = criterion.violated(avg as f32) {
                            if violated {
                                Status::Fail
                            } else {
                                Status::Pass
                            }
                        } else {
                            Status::Complete
                        };

                        (status, Some(format!("{:.2}mJ/s average", avg)))
                    },

                    EnergyStat::Max => {
                        let samples = self.energy_metrics.get(criterion.get_meter()).unwrap();
                        let max = samples.iter()
                            .copied()
                            .fold(0f32, |curr, n| if n > curr { n } else { curr });

                        let status = if let Some(violated) = criterion.violated(max as f32) {
                            if violated {
                                Status::Fail
                            } else {
                                Status::Pass
                            }
                        } else {
                            Status::Complete
                        };

                        (status, Some(format!("{:.2}mJ/s max", max)))
                    },

                    EnergyStat::Min => {
                        let samples = self.energy_metrics.get(criterion.get_meter()).unwrap();
                        let min = if samples.len() > 0 {
                            samples.iter()
                                .copied()
                                .fold(f32::MAX, |curr, n| if n < curr { n } else { curr })
                        } else {
                            0f32
                        };

                        let status = if let Some(violated) = criterion.violated(min as f32) {
                            if violated {
                                Status::Fail
                            } else {
                                Status::Pass
                            }
                        } else {
                            Status::Complete
                        };

                        (status, Some(format!("{:.2}mJ/s min", min)))
                    },
                }
            },

            Criterion::ParallelTrace(trace_criterion) => {
                let execution_t0 = self.exec_result
                    .as_ref()
                    // Evaluation results are only relevant when the exec_result is Ok(...).
                    .expect("Attempted to evaluate criterion when execution result failed")
                    .get_start();
                if let Some(aligned_traces) = trace_criterion.align(*execution_t0, self.parallel_traces.as_slice()) {
                    let count = aligned_traces.len();
                    let mut message = "Satisfied by: ".to_string();
                    let it = aligned_traces.into_iter()
                        .map(|t| format!("@{:?}", t.get_time() - *execution_t0));
                    for (msg, no) in it.zip(1..) {
                        message.push_str(&msg);
                        if no < count {
                            message.push_str(" → ");
                        }
                    }
                    (Status::Pass, Some(message))
                } else {
                    (Status::Fail, None)
                }
            },

            Criterion::SerialTrace(trace_criterion) => {
                let execution_t0 = self.exec_result
                    .as_ref()
                    .expect("Attempted to evaluate serial tracing criterion when execution result failed")
                    .get_start();
                if let Some(aligned_traces) = trace_criterion.align(*execution_t0, self.serial_traces.as_slice()) {
                    let count = aligned_traces.len();
                    let mut message = "Satisfied by: ".to_string();
                    let it = aligned_traces.into_iter()
                        .map(|t| format!("@{:?}", t.get_offset(*execution_t0)));
                    for (msg, no) in it.zip(1..) {
                        message.push_str(&msg);
                        if no < count {
                            message.push_str(" → ");
                        }
                    }
                    (Status::Pass, Some(message))
                } else {
                    (Status::Fail, None)
                }
            },
        }
    }
}

impl Display for Evaluation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}\t", self.test.get_id())?;
        match self.outcome() {
            Status::Error => write!(f, "Error ({})", self.get_exec_result().as_ref().unwrap_err()),
            outcome => write!(f, "{} (in {:?})", outcome, self.get_exec_result().as_ref().unwrap().duration()),
        }?;
        write!(f, "\n")?;

        if let Some(ref spec) = self.spec {
            write!(f, "{}\n", spec)?;
        }

        if let Ok(ref execution) = self.exec_result {
            if self.device_responses.len() > 0 {
                write!(f, "  IO responses:\n")?;
                for response in &self.device_responses {
                    write!(f, "    @{:?}\t{}\n", response.get_offset(*execution.get_start()), response)?
                }
            }

            if self.energy_metrics.len() > 0 {
                write!(f, "  Energy metering:\n")?;
                for (meter_id, samples) in &self.energy_metrics {
                    write!(f, "    {:<10} ({} samples)\n", meter_id, samples.len())?;
                }
            }

            if self.parallel_traces.len() > 0 {
                write!(f, "  Traces:\n")?;
                for trace in &self.parallel_traces {
                    let spec = self.spec.as_ref().unwrap();
                    let trace_point_name = spec.trace_point_name(trace.get_id())
                        .map(|s| s.as_str())
                        .unwrap_or("UNTRACKED?");
                    write!(f, "    {}\t(data: {})\t@{:?}\n",
                           trace_point_name,
                           trace.get_extra(),
                           trace.get_offset(*execution.get_start()))?;

                    if spec.trace_point_name(trace.get_id()).is_none() {
                        write!(f, "{}\n", trace)?;
                    }
                }
            }

            write!(f, "\n")?;

            // Show criteria results.
            write!(f, "=== Criteria summary:\n")?;
            for criterion in self.test.get_criteria() {
                let (status, opt_message) = self.evaluate(criterion);
                write!(f, "  - {} ({})\n", criterion, status)?;
                if let Some(ref message) = opt_message {
                    write!(f, "    Message: {}\n", message)?;
                }
            }
        }

        Ok(())
    }
}