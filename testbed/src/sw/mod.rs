/*! Software integration with devices under test.
 */

pub mod error;
pub mod platform;

use std::collections::HashSet;
use std::convert::From;
use std::convert::TryFrom;
use std::fmt;
use std::fmt::{Debug, Display};

use flexbed_common::sw::instrument::Spec;

use error::Error;

type Result<T> = std::result::Result<T, Error>;

/// Embedded systems software platform.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Platform {
    /// Tock OS
    Tock,
}

impl TryFrom<&str> for Platform {
    type Error = String;

    fn try_from(s: &str) -> std::result::Result<Self, Self::Error> {
        use Platform::*;
        match s {
            "tock" => Ok(Tock),
            _ => Err(format!("'{}' is not a valid platform", s)),
        }
    }
}

impl Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Platform::Tock => write!(f, "Tock OS"),
        }
    }
}

impl From<Platform> for String {
    fn from(platform: Platform) -> String {
        match platform {
            Platform::Tock => "Tock OS".to_string(),
        }
    }
}

/// A platform that supports loading software modules, apps, etc.
pub trait PlatformSupport: Debug {
    /// Returns the target's platform.
    fn platform(&self) -> Platform;

    /// Load software onto the device.
    fn load(&self, name: &str) -> Result<()>;

    /// Remove software from the target.
    fn unload(&self, name: &str) -> Result<()>;

    /// Returns an iterator over the platform's loaded software.
    fn loaded_software(&self) -> HashSet<String>;

    /// Apply reconfigured platform software to the target.
    fn reconfigure(&self, trace_points: &Vec<String>) -> Result<Spec> {
        let _ = trace_points;
        Err(Error::Unsupported)
    }
}
