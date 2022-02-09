//! Process and evaluate test data.

use std::time::Duration;
use std::fmt::{self, Display};

use crate::criteria::{
    Criterion,
    GPIOCriterion,
    EnergyStat,
};
use crate::testbed::Observation;

/// Judged outcome.
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

/// Result of evaluating a single `Criterion`.
pub struct Outcome<'a> {
    criterion: &'a Criterion,
    status: Status,
    message: Option<String>,
}

impl<'a> Outcome<'a> {
    /// Create a new `Outcome`.
    pub fn new(
        criterion: &'a Criterion,
        status: Status,
        message: Option<String>
    ) -> Outcome<'a> {
        Outcome {
            criterion,
            status,
            message,
        }
    }

    /// Return the satisfaction status of the criterion.
    pub fn status(&self) -> Status {
        self.status
    }

    /// Return the accompanying message from the evaluation of the criterion.
    pub fn message(&self) -> Option<&String> {
        self.message.as_ref()
    }
}

/// Result of evaluating test data.
pub struct Evaluation<'a> {
    status: Status,
    outcomes: Vec<Outcome<'a>>,
    data: &'a Observation,
}

impl<'a> Evaluation<'a> {
    /// Create a new `Evaluation`.
    pub fn new(
        status: Status,
        outcomes: Vec<Outcome<'a>>,
        data: &'a Observation,
    ) -> Evaluation<'a>
    {
        Evaluation {
            status,
            outcomes,
            data,
        }
    }
}

impl<'a> Display for Evaluation<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}\t", self.data.source_test().get_id())?;
        match self.status {
            Status::Error => write!(f, "Error ({})", self.data.execution_result().as_ref().unwrap_err()),
            test_outcome => write!(f, "{} (in {:?})", test_outcome, self.data.execution_result().as_ref().unwrap().duration()),
        }?;
        write!(f, "\n")?;

        Ok(())
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
        match observation.execution_result() {
            Ok(_execution_info) => {
                let criteria = observation.source_test().get_criteria();
                let outcomes: Vec<_> = criteria.iter()
                    .map(|criterion| evaluate(criterion, observation))
                    .collect();

                // Summarize the evaluation's outcome by inspecting the component criteria's outcomes.
                let overall_status = outcomes.iter()
                    .map(|outcome| outcome.status())
                    .fold(Status::Complete, |overall_status, outcome_status| {
                        match overall_status {
                            // Any other status has higher priority over Complete.
                            Status::Complete => outcome_status,

                            // Only Fail and Error have priority.
                            Status::Pass => match outcome_status {
                                Status::Fail => Status::Fail,
                                Status::Error => Status::Error,
                                _ => Status::Pass,
                            },

                            // Only an error can change a Fail.
                            Status::Fail => match outcome_status {
                                Status::Error => Status::Error,
                                _ => Status::Fail,
                            },

                            // Nothing can change an Error.
                            Status::Error => overall_status,
                        }
                    });

                Evaluation::new(overall_status, outcomes, observation)
            },

            Err(_err) => Evaluation::new(Status::Error, Vec::new(), observation),
        }
    }
}

/// Evaluate criterion defined within Clockwise.
pub fn evaluate<'a>(criterion: &'a Criterion, data: &Observation) -> Outcome<'a> {
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
                    let samples = data.energy_metrics()
                        .get(criterion.get_meter())
                        .unwrap();

                    let execution_duration = data.execution_result()
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
                    let samples = data.energy_metrics().get(criterion.get_meter()).unwrap();
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
                    let samples = data.energy_metrics().get(criterion.get_meter()).unwrap();
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
                    let samples = data.energy_metrics().get(criterion.get_meter()).unwrap();
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
            let execution_t0 = data.execution_result()
                .as_ref()
                .expect("Attempted to evaluate serial tracing criterion when execution result failed")
                .get_start();
            if let Some(aligned_traces) = trace_criterion.align(execution_t0, data.traces().as_slice()) {
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

    Outcome::new(criterion, status, message)
}
