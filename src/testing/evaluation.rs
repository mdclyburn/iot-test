//! Insights into test executions.

use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::time::Duration;

use super::{Criterion,
            GPIOCriterion,
            EnergyStat,
            Execution,
            Response,
            Result,
            Test};

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
    exec_result: Result<Execution>,
    device_responses: Vec<Response>,
    energy_metrics: HashMap<String, Vec<f32>>,
}

impl Evaluation {
    pub fn new(test: &Test,
               exec_result: Result<Execution>,
               device_responses: Vec<Response>,
               energy_metrics: HashMap<String, Vec<f32>>
    ) -> Evaluation {
        Evaluation {
            test: test.clone(),
            exec_result,
            device_responses,
            energy_metrics,
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
            }
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

        if let Ok(ref execution) = self.exec_result {
            if self.device_responses.len() > 0 {
                write!(f, "  IO responses:")?;
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
