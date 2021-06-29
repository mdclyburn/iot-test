use std::collections::HashMap;
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
use crate::sw::{Platform, PlatformSupport};
use crate::sw::platform;
use crate::testing::testbed::Testbed;

use super::{Result,
            TestbedConfigReader};
use super::error::Error;

const CONFIG_VERSION: i64 = 1;

#[derive(Debug)]
pub struct JSONTestbedParser {
    config_path: PathBuf,
}

impl JSONTestbedParser {
    pub fn new(config_path: &Path) -> JSONTestbedParser {
        JSONTestbedParser {
            config_path: config_path.to_path_buf(),
        }
    }

    fn parse_gpio(&self, json: &JSONValue) -> Result<IOConfig> {
        let io_json = json["io"].as_object()
            .ok_or(Error::Format("Missing 'io' object.".to_string()))?;
        serde_json::from_value(serde_json::Value::Object(io_json.clone()))
            .map_err(|e| Error::Format(format!("IO configuration parsing failed: {}", e)))
    }

    fn parse_energy(&self, mapping: &Mapping, json: &JSONValue) -> Result<HashMap<String, Box<dyn EnergyMetering>>> {
        let mut meters = HashMap::new();
        let json_meters = json["meters"].as_array()
            .ok_or(Error::Format("Energy 'meters' must be an array.".to_string()))?;
        for json_meter in json_meters {
            let name = json_meter["name"].as_str()
                .ok_or(Error::Format("Energy meter is missing a name.".to_string()))?;
            let driver = json_meter["driver-id"].as_str()
                .ok_or(Error::Format("Energy meter is missing a 'driver-id'.".to_string()))?;
            let props = json_meter["driver-props"].as_object()
                .ok_or(Error::Format(format!("Energy meter '{}' is missing 'driver-props'.", name)))?;

            let meter: Box<dyn EnergyMetering> = match driver {
                "ina219" => Ok(Box::<hw::INA219>::new(hw::INA219::from_json(mapping, serde_json::Value::Object(props.clone()))?)),
                _ => Err(Error::Unsupported),
            }?;

            meters.insert(name.to_string(), meter);
        }

        Ok(meters)
    }

    fn parse_platform(&self, platform_json: &JSONValue) -> Result<Box<dyn PlatformSupport>> {
        let platform_id = platform_json["id"].as_str()
            .ok_or(Error::Format("Platform missing 'id' string.".to_string()))?;
        let platform: Box<dyn PlatformSupport> = match platform_id {
            "tock" => Box::<platform::Tock>::new(platform::Tock::from_json(platform_json)?),
            _ => return Err(Error::Unsupported),
        };

        Ok(platform)
    }
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
            .and_then(|ver| if ver == CONFIG_VERSION {
                Ok(())
            } else {
                let msg = format!(
                    "Configuration not compatible (provided: {}, required: {}).",
                    ver,
                    CONFIG_VERSION);
                Err(Error::Format(msg))
            })?;

        // Host and target I/O.
        let mapping = self.parse_gpio(&json)?
            .create_mapping()?;
        // Energy metering.
        let energy_meters = self.parse_energy(&mapping, &json["energy"])?;
        // Software platform support.
        let platform_support = self.parse_platform(&json["platform"])?;

        let testbed = Testbed::new(
            mapping,
            platform_support,
            energy_meters);

        Ok(testbed)
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

trait JSONPlatform: Sized {
    fn from_json(props: &JSONValue) -> Result<Self> {
        Err(Error::Unsupported)
    }
}

#[derive(Deserialize)]
struct TockPlatformConfig {
    #[serde(alias = "tockloader-path")]
    tockloader_path: String,
    #[serde(alias = "repo-path")]
    repo_path: String,
    #[serde(alias = "application-path")]
    app_path: String,
    board: String,
}

impl JSONPlatform for platform::Tock {
    fn from_json(props: &JSONValue) -> Result<Self> {
        let config: TockPlatformConfig = serde_json::from_value(props.clone())
            .map_err(|e| Error::Format(format!("Tock support: deserialization error: {}", e)))?;
        let tock_support = platform::Tock::new(
            config.board.as_str(),
            Path::new(&config.tockloader_path),
            Path::new(&config.repo_path),
            Path::new(&config.app_path));

        Ok(tock_support)
    }
}
