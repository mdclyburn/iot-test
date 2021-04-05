//! Evaluate test executions

use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::time::Duration;

use super::{Criterion,
            GPIOCriterion,
            EnergyCriterion,
            Execution,
            Response,
            Result,
            Test};

/// Summary of an `Evaluation`.
#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub enum Status {
    Complete,
    Pass,
    Fail,
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

    pub fn get_exec_result(&self) -> &Result<Execution> {
        &self.exec_result
    }

    pub fn outcome(&self) -> Status {
        if self.exec_result.is_err() {
            Status::Error
        } else {
            Status::Complete
        }
    }

    fn evaluate(&self, criterion: &Criterion) -> (Status, Option<String>) {
        match criterion {
            Criterion::GPIO(criterion) => {
                match criterion {
                    GPIOCriterion::Any(_pin) => (Status::Complete, None),
                }
            },

            Criterion::Energy(criterion) => {
                match criterion {
                    EnergyCriterion::Consumption(meter_id) => {
                        // let sample_rate_us = self.test.get_energy_sampling_rate()
                        //     .as_micros();
                        // let frac = sample_rate_us as f64 / Duration::from_secs(1).as_micros() as f64;

                        // Should exist in map because criterion stated it should be tracked.
                        let samples = self.energy_metrics.get(meter_id)
                            .unwrap();

                        let actual_length = self.exec_result
                            .as_ref()
                            // Evaluation results are only relevant when the exec_result is Ok(...).
                            .expect("Attempted to evaluate criterion when execution result failed")
                            .duration();
                        let sample_count = samples.len();
                        let sampling_rate = actual_length / sample_count as u32;
                        let sample_weight = sampling_rate.as_micros() as f64 / Duration::from_secs(1).as_micros() as f64;

                        let mut total = 0f64;
                        for sample in samples.iter().copied() {
                            // mJ/s * fraction of the second the sample accounts for
                            total += sample as f64 * sample_weight;
                        }

                        (Status::Complete, Some(format!("{:.2}mJ consumed", total)))
                    },

                    EnergyCriterion::Average(meter_id) => {
                        let samples = self.energy_metrics.get(meter_id).unwrap();
                        // ASSUMPTION: timer intervals represented by all samples are equal.
                        let avg: f32 = samples.iter().sum::<f32>() / samples.len() as f32;
                        (Status::Complete, Some(format!("{:.2}mJ/s average", avg)))
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
