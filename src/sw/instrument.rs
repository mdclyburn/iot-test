//! Platform instrumentation support.

use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

use super::Result;
// use error::Error;

use json;
use json::JsonValue;

const SPEC_VERSION: u32 = 1;

/// Information about a platform build.
pub struct Spec {
    name_value: HashMap<String, u8>,
    value_name: HashMap<u8, String>,
}

impl Spec {
    pub fn new<'a, T>(trace_points: T) -> Spec
    where
        T: IntoIterator<Item = &'a str>
    {
        let name_value: HashMap<String, u8> = trace_points.into_iter()
            .map(|s| s.to_string())
            .zip(1..) // assign values to each trace point
            .collect();
        let value_name = name_value.iter()
            .map(|(n, v)| (*v, n.clone()))
            .collect();

        Spec {
            name_value,
            value_name,
        }
    }

    #[allow(dead_code)]
    pub fn value_of(&self, trace_point_name: &str) -> Option<u8> {
        self.name_value.get(trace_point_name)
            .map(|val| *val)
    }

    #[allow(dead_code)]
    pub fn name_of(&self, value: u8) -> Option<&String> {
        self.value_name.get(&value)
    }

    #[allow(dead_code)]
    pub fn write(&self, out_path: &Path) -> Result<()> {
        let points: Vec<JsonValue> = self.name_value.iter()
            .map(|(name, value)| json::object! { name: name.clone(), value: *value })
            .collect();
        let obj = json::object! {
            "_version": SPEC_VERSION,
            "trace-points": points,
        };

        {
            let mut file = File::create(out_path)?;
            obj.write(&mut file)?;

            use std::io::Write;
            file.flush()?;
        }

        Ok(())
    }
}
