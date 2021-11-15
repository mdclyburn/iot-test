//! Result output formatting.

use std::fmt::Debug;
use std::collections::HashMap;
use std::time::Instant;

use crate::test::Response;
use crate::trace::SerialTrace;

/// Writer for raw data from tests.
pub trait DataWriter: Debug {
    /// Save evaluation data.
    fn save_output(&self,
                   data_id: &str,
                   responses: &[Response],
                   traces: &[SerialTrace],
                   energy: &HashMap<String, Vec<(Instant, f32)>>)
                   -> Result<(), String>;
}
