/*! Software integration with devices under test.
 */

pub mod application;
pub mod error;
pub mod instrument;
pub mod platform;

use std::collections::HashSet;
use std::convert::From;
use std::convert::TryFrom;
use std::fmt;
use std::fmt::{Debug, Display};

use application::Application;
use instrument::Spec;
use error::Error;

type Result<T> = std::result::Result<T, Error>;

/// Embedded systems software platform.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Platform {
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
    fn load(&self, app: &Application) -> Result<()>;

    /// Remove software from the target.
    fn unload(&self, app_id: &str) -> Result<()>;

    /// Returns an iterator over the platform's loaded software.
    fn loaded_software(&self) -> HashSet<String>;

    /// Apply reconfigured platform software to the target.
    fn reconfigure(&self, trace_points: &Vec<String>) -> Result<Spec> {
        let _ = trace_points;
        Err(Error::Unsupported)
    }
}
