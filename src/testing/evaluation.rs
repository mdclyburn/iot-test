//! Evaluate test executions

use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;

use super::{Execution, Response, Result, Test};

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
    test_id: String,
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
            test_id: test.get_id().to_string(),
            exec_result,
            device_responses,
            energy_metrics,
        }
    }

    pub fn get_outcome(&self) -> Status {
        if self.exec_result.is_err() {
            Status::Error
        } else {
            // more nuanced info here
            Status::Complete
        }
    }

    pub fn get_exec_result(&self) -> &Result<Execution> {
        &self.exec_result
    }
}

impl Display for Evaluation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}\t", self.test_id)?;
        match self.get_outcome() {
            Status::Error => write!(f, "Error ({})", self.get_exec_result().as_ref().unwrap_err()),
            outcome => write!(f, "{} (in {:?})", outcome, self.get_exec_result().as_ref().unwrap().get_duration()),
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
        }

        Ok(())
    }
}
