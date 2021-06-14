//! Platform instrumentation support.

use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::fs::File;
use std::path::Path;

use super::Result;
// use error::Error;

use json;
use json::JsonValue;

const SPEC_VERSION: u32 = 1;

/// Information about a platform build.
#[derive(Clone, Debug)]
pub struct Spec {
    name_value: HashMap<String, u16>,
    value_name: HashMap<u16, String>,
}

impl Spec {
    pub fn new<'a, T>(trace_points: T) -> Spec
    where
        T: IntoIterator<Item = &'a str>
    {
        let name_value: HashMap<String, u16> = trace_points.into_iter()
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
    pub fn trace_point_value(&self, name: &str) -> Option<u16> {
        self.name_value.get(name)
            .map(|val| *val)
    }

    #[allow(dead_code)]
    pub fn trace_point_name(&self, value: u16) -> Option<&String> {
        self.value_name.get(&value)
    }

    pub fn id_bit_length(&self) -> u8 {
        let allocated = self.name_value.len();
        for pow in 1..16 {
            if (1u16 << pow) > allocated as u16 {
                return pow;
            }
        }

        panic!("ID bit length too long.");
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

impl Display for Spec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Platform specification:\n")?;
        for (name, value) in &self.name_value {
            write!(f, "  {} => {:2}\n", name, value)?;
        }

        Ok(())
    }
}
