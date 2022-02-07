//! Process and evaluate test data.

use std::time::Duration;
use std::fmt::{self, Display};

use crate::criteria::{
    Criterion,
    GPIOCriterion,
    EnergyStat,
};
use crate::sw::instrument::Spec;
use crate::test::{
    Execution,
    Response,
    Test,
};
use crate::testbed::Observation;
use crate::trace::SerialTrace;

/// Judged outcome of a test.
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

/// A judge for test data corresponding to tests executed by the testbed.
pub trait Evaluator {
    /// Evaluate a single observation that arose from executing a test.
    fn evaluate<'a>(&self, observation: &'a Observation) -> Evaluation<'a>;
}

/// Result of evaluating test data.
pub struct Evaluation<'a> {
    data: &'a Observation,
    outcomes: Vec<(&'a Criterion, (Status, Option<String>))>,
}

impl<'a> Evaluation<'a> {
    /// Create a new `Evaluation`.
    pub fn new(data: &'a Observation,
               outcomes: Vec<(&'a Criterion, (Status, Option<String>))>,
    ) -> Evaluation<'a>
    {
        Evaluation {
            data,
            outcomes,
        }
    }
}

/// Basic, built-in evaluator.
pub struct StandardEvaluator;

impl StandardEvaluator {
    /// Create a new `StandardEvaluator`.
    pub fn new() -> StandardEvaluator {
        StandardEvaluator
    }
}

impl Evaluator for StandardEvaluator {
    fn evaluate<'a>(&self, observation: &'a Observation) -> Evaluation<'a> {
        let criteria = observation.source_test().get_criteria();
        let mut outcomes = Vec::new();

        for criterion in criteria {
            let (status, message) = match criterion {
                Criterion::GPIO(criterion) => {
                    match criterion {
                        GPIOCriterion::Any(_pin) => (Status::Complete, None),
                    }
                },

                Criterion::Energy(criterion) => {
                    match criterion.get_stat() {
                        EnergyStat::Total => {
                            // Should exist in map because criterion stated it should be tracked.
                            let samples = observation.energy_metrics()
                                .get(criterion.get_meter())
                                .unwrap();

                            let execution_duration = observation.execution_result()
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
                            for sample in samples.iter().map(|(_t, s)| s).copied() {
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
                            let samples = observation.energy_metrics().get(criterion.get_meter()).unwrap();
                            // ASSUMPTION: timer intervals represented by all samples are equal.
                            let avg: f32 = samples.iter().map(|(_t, s)| s).sum::<f32>() / samples.len() as f32;

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
                            let samples = observation.energy_metrics().get(criterion.get_meter()).unwrap();
                            let max = samples.iter()
                                .map(|(_t, sample)| sample)
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
                            let samples = observation.energy_metrics().get(criterion.get_meter()).unwrap();
                            let min = if samples.len() > 0 {
                                samples.iter()
                                    .map(|(_t, sample)| sample)
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

                Criterion::SerialTrace(trace_criterion) => {
                    let execution_t0 = observation.execution_result()
                        .as_ref()
                        .expect("Attempted to evaluate serial tracing criterion when execution result failed")
                        .get_start();
                    if let Some(aligned_traces) = trace_criterion.align(execution_t0, observation.traces().as_slice()) {
                        let count = aligned_traces.len();
                        let mut message = "Satisfied by: ".to_string();
                        let it = aligned_traces.into_iter()
                            .map(|t| format!("@{:?}", t.get_offset(execution_t0)));
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
            };

            outcomes.push((criterion, (status, message)));
        }

        Evaluation::new(observation, outcomes)
    }
}
