use std::path::{Path, PathBuf};
use std::process::Command;

use super::Error;
use super::Loadable;
use super::Platform;
use super::Result;

pub struct Tock {
    tockloader_path: PathBuf,
}

impl Tock {
    pub fn new(tockloader_path: &Path) -> Tock {
        Tock {
            tockloader_path: tockloader_path.to_path_buf(),
        }
    }
}

impl Loadable for Tock {
    fn load(&self, path: &Path) -> Result<()> {
        let tockloader_path_str = self.tockloader_path.to_str()
            .ok_or(Error::Other(format!("cannot convert '{}' to Unicode", self.tockloader_path.display())))?;
        let app_path_str = path.to_str()
            .ok_or(Error::Other(format!("cannot convert '{}' to Unicode", path.display())))?;

        let output = Command::new(tockloader_path_str)
            .args(&["install", app_path_str])
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            Err(Error::Load(output))
        }
    }

    fn unload(&self) -> Result<()> {
        Ok(())
    }

    fn platform(&self) -> Platform {
        Platform::Tock
    }
}
