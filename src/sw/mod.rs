/*! Software integration with devices under test.
 */

pub mod application;
pub mod error;
pub mod platform;

use std::convert::From;
use std::fmt;
use std::fmt::{Debug, Display};
use std::path::Path;

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

/// A platform that supports loading software.
pub trait Loadable: Debug {
    /// Load software from the given path.
    fn load(&self, path: &Path) -> Result<()>;

    /// Remove software from the device.
    fn unload(&self) -> Result<()>;

    /// Returns the device's platform.
    fn platform(&self) -> Platform;
}
