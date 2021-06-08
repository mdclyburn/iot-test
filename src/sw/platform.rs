//! Multi-platform support interfaces.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::Error;
use super::PlatformSupport;
use super::Platform;
use super::Result;
use super::application::Application;

pub struct Configuration {
    trace_points: Vec<String>,
}

impl Configuration {
    pub fn new<'a, T>(trace_points: T) -> Configuration
    where
        T: IntoIterator<Item = &'a str>
    {
        Configuration {
            trace_points: trace_points.into_iter()
                .map(|s| s.to_string())
                .collect()
        }
    }

    pub fn get_trace_points(&self) -> &Vec<String> {
        &self.trace_points
    }
}

/// Testbed support for the Tock OS platform.
#[derive(Clone, Debug)]
pub struct Tock {
    tockloader_path: PathBuf,
    loaded_apps: HashSet<String>,
}

impl Tock {
    pub fn new(tockloader_path: &Path) -> Tock {
        Tock {
            tockloader_path: tockloader_path.to_path_buf(),
            loaded_apps: HashSet::new(),
        }
    }
}

impl PlatformSupport for Tock {
    fn load(&mut self, app: &Application) -> Result<()> {
        let tockloader_path_str = self.tockloader_path.to_str()
            .ok_or(Error::Other(format!("cannot convert '{}' to Unicode", self.tockloader_path.display())))?;
        let path = app.get_for(self.platform())?;
        let app_path_str = path.to_str()
            .ok_or(Error::Other(format!("cannot convert '{}' to Unicode", path.display())))?;

        let output = Command::new(tockloader_path_str)
            .args(&["install", app_path_str])
            .output()?;

        if output.status.success() {
            self.loaded_apps.insert(app.get_id().to_string());
            Ok(())
        } else {
            Err(Error::Tool(output))
        }
    }

    fn unload(&mut self, app_id: &str) -> Result<()> {
        // No need to remove what's not there.
        if !self.loaded_apps.remove(app_id) {
            Ok(())
        } else {
            let tockloader_path_str = self.tockloader_path.to_str()
                .ok_or(Error::Other(format!("cannot convert '{}' to Unicode", self.tockloader_path.display())))?;

            let output = Command::new(tockloader_path_str)
                .args(&["uninstall"])
                .output()?;

            if output.status.success() {
                Ok(())
            } else {
                // Question: what state is the device in if we fail?
                Err(Error::Tool(output))
            }
        }
    }

    fn loaded_software<'a>(&'a self) -> Box<dyn Iterator<Item = &'a String> + 'a> {
        Box::new(self.loaded_apps.iter())
    }

    fn platform(&self) -> Platform {
        Platform::Tock
    }
}
