/*! Software integration with devices under test.
 */

pub mod error;
pub mod tock;

use std::path::Path;

use error::Error;

type Result<T> = std::result::Result<T, Error>;

/// Embedded systems software platform.
pub enum Platform {
    Tock,
}

/// A platform that supports loading software.
pub trait Loadable {
    /// Load software from the given path.
    fn load(&self, path: &Path) -> Result<()>;

    /// Remove software from the device.
    fn unload(&self) -> Result<()>;

    /// Returns the device's platform.
    fn platform(&self) -> Platform;
}
