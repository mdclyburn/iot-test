//! Insights into test executions.

use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::time::{Duration, Instant};

use clockwise_common::criteria::{
    Criterion,
    GPIOCriterion,
    EnergyStat,
};
use clockwise_common::sw::instrument::Spec;
use clockwise_common::test::{
    Execution,
    Response,
    Test
};
use clockwise_common::testbed::TestbedError;
use clockwise_common::trace::SerialTrace;

type Result<T> = std::result::Result<T, TestbedError>;

/// In-depth information about a test execution.
#[derive(Debug)]
pub struct Evaluation {
    test: Test,
    spec: Option<Spec>,
    exec_result: Result<Execution>,
    device_responses: Vec<Response>,
    serial_traces: Vec<SerialTrace>,
    energy_metrics: HashMap<String, Vec<(Instant, f32)>>,
}

impl Evaluation {
    pub fn new(test: &Test,
               spec: &Spec,
               exec_result: Result<Execution>,
               device_responses: Vec<Response>,
               serial_traces: Vec<SerialTrace>,
               energy_metrics: HashMap<String, Vec<(Instant, f32)>>) -> Evaluation
    {
        Evaluation {
            test: test.clone(),
            spec: Some(spec.clone()),
            exec_result,
            device_responses,
            serial_traces,
            energy_metrics,
        }
    }

    /// Create an evaluation that fails due to an error during testing.
    pub fn failed(test: &Test, spec: Option<&Spec>, error: TestbedError) -> Evaluation {
        Evaluation {
            test: test.clone(),
            spec: spec.map(|s| s.clone()),
            exec_result: Err(error),
            device_responses: Vec::new(),
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
                    write!(f, "    @{:?}\t{}\n", response.get_offset(execution.get_start()), response)?
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
