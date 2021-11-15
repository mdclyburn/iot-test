//! CSV output formatting for data.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use clockwise_common::output::DataWriter;
use clockwise_common::trace::SerialTrace;
use clockwise_common::test::Response;

#[derive(Debug)]
pub struct CSVDataWriter {
    base_path: PathBuf,
}

impl CSVDataWriter {
    pub fn new(base_path: &Path) -> CSVDataWriter {
        CSVDataWriter {
            base_path: PathBuf::from(base_path),
        }
    }
}

impl DataWriter for CSVDataWriter {
    fn save_output(&self,
                   data_id: &str,
                   responses: &[Response],
                   traces: &[SerialTrace],
                   energy: &HashMap<String, Vec<f32>>)
                   -> Result<(), String>
    {
        Ok(())
    }
}
