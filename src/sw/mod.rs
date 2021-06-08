/*! Software integration with devices under test.
 */

pub mod application;
pub mod error;
pub mod instrument;
pub mod platform;

use std::convert::From;
use std::fmt;
use std::fmt::{Debug, Display};

use application::Application;
use error::Error;

type Result<T> = std::result::Result<T, Error>;

/// Embedded systems software platform.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Platform {
    Tock,
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
    fn load(&mut self, app: &Application) -> Result<()>;

    /// Remove software from the target.
    fn unload(&mut self, app_id: &str) -> Result<()>;

    /// Returns an iterator over the platform's loaded software.
    fn loaded_software<'a>(&'a self) -> Box<dyn Iterator<Item = &'a String> + 'a>;

    /// Apply reconfigured platform software to the target.
    fn reconfigure(&self, trace_points: &Vec<String>) -> Result<()> {
        let _ = trace_points;
        Err(Error::Unsupported)
    }
}
