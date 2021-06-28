use std::convert::TryFrom;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json;
use serde_json::Value as JSONValue;

use crate::comm::{Direction,
                  Class as SignalClass};
use crate::device::Device;
use crate::facility::EnergyMetering;
use crate::hw;
use crate::io::Mapping;
use crate::sw::Platform;
use crate::testing::testbed::Testbed;

use super::{Result,
            TestbedConfigReader};
use super::error::Error;

const CONFIG_VERSION: i64 = 1;

pub struct JSONTestbedParser {
    config_path: PathBuf,
}

impl JSONTestbedParser {
    pub fn new(&self, config_path: &Path) -> JSONTestbedParser {
        JSONTestbedParser {
            config_path: config_path.to_path_buf(),
        }
    }

    fn parse_gpio(&self, json: &JSONValue) -> Result<IOConfig> {
        let io_json = json["io"].as_object()
            .ok_or(Error::Format("Missing 'io' object.".to_string()))?;
        serde_json::from_value(json.clone())
            .map_err(|e| Error::Format(format!("IO configuration parsing failed: {}", e)))
    }

    // fn parse_energy(&self, json: &JSONValue) -> Result<HashMap<String, Box<dyn EnergyMetering>>> {
    // }
}

impl TestbedConfigReader for JSONTestbedParser {
    fn create(&self) -> Result<Testbed> {
        let mut text = String::new();
        let mut file = File::open(self.config_path.as_path())?;
        file.read_to_string(&mut text)?;

        let json: JSONValue = serde_json::from_str(&text)
            .map_err(|e| Error::Format(format!("JSON parsing failure: {}", e)))?;

        // Check file version.
        json["_version"].as_i64()
            .ok_or(Error::Format("Missing '_version' specifier.".to_string()))
            .and_then(|ver| if ver != CONFIG_VERSION {
                Ok(())
            } else {
                let msg = format!(
                    "Configuration not compatible (provided: {}, required: {}).",
                    ver,
                    CONFIG_VERSION);
                Err(Error::Format(msg))
            })?;

        let _platform = json["platform"].as_str()
            .ok_or(Error::Format("Configuration does not specify 'platform'.".to_string()))
            .and_then(|p_str| Platform::try_from(p_str).map_err(|e| Error::Format(e)))?;

        let _mapping = self.parse_gpio(&json)?
            .create_mapping()?;

        Err(Error::Format("".to_string()))
    }
}

#[derive(Deserialize)]
struct IOConfig {
    gpio: Vec<PinConfig>,
    trace_pins: Vec<u8>,
}

impl IOConfig {
    fn create_mapping(&self) -> Result<Mapping> {
        let mut device_io: Vec<(u8, (Direction, SignalClass))> = Vec::new();
        for pcfg in &self.gpio {
            let dir = Direction::try_from(pcfg.direction.as_str())
                .map_err(|e| Error::Format(e.to_string()))?;
            let sig = SignalClass::try_from(pcfg.signal.as_str())
                .map_err(|e| Error::Format(e.to_string()))?;

            device_io.push((pcfg.dpin, (dir, sig)));
        }

        let device = Device::new(&device_io);
        let pin_conns: Vec<_> = self.gpio.iter().map(|pcfg| (pcfg.tpin, pcfg.dpin))
            .collect();
        let it_trace_pins = self.trace_pins.iter();

        let mapping = Mapping::new(&device, &pin_conns, it_trace_pins)
            .map_err(|e| Error::Format(format!("IO mapping error: {}", e)))?;

        Ok(mapping)
    }
}

#[derive(Deserialize)]
struct PinConfig {
    dpin: u8,
    tpin: u8,
    direction: String,
    signal: String,
}

trait JSONHardware: Sized {
    fn from_json(mapping: &Mapping, json: JSONValue) -> Result<Self> {
        Err(Error::Unsupported)
    }
}

impl JSONHardware for hw::INA219 {
    fn from_json(mapping: &Mapping, json: JSONValue) -> Result<Self> {
        let i2c = mapping.get_i2c()?;
        let address = json["i2c-address"].as_i64()
            .ok_or(Error::Format("INA219: missing 'i2c-address' property.".to_string()))
            .and_then(|addr| u8::try_from(addr)
                                 .map_err(|_e| Error::Format("INA219: 'i2c-address' is not valid.".to_string())))?;

        hw::INA219::new(i2c, address)
            .map_err(|e| Error::Driver(e.to_string()))
    }
}
