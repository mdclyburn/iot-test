//! Multi-platform support interfaces.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::application::Application;
use super::error::Error;
use super::instrument::Spec;
use super::Platform;
use super::PlatformSupport;
use super::Result;

/// Testbed support for the Tock OS platform.
#[derive(Clone, Debug)]
pub struct Tock {
    tockloader_path: PathBuf,
    loaded_apps: HashSet<String>,
    source_path: PathBuf,
}

impl Tock {
    /// Create a new Tock platform instance.
    pub fn new(tockloader_path: &Path, source_path: &Path) -> Tock {
        Tock {
            tockloader_path: tockloader_path.to_path_buf(),
            loaded_apps: HashSet::new(),
            source_path: source_path.to_path_buf(),
        }
    }

    /// Build the Tock OS.
    fn build(&self, enable_tracing: Option<&Path>) -> Result<()> {
        // NOTICE: forcing use of the Hail board configuration.
        let make_work_dir = self.source_path.clone()
            .join("boards/hail");

        let mut env_vars: Vec<(String, String)> = Vec::new();
        if let Some(spec_path) = enable_tracing {
            env_vars.push(("TRACE_SPEC_PATH".to_string(),
                           spec_path.to_str().unwrap().to_string()));
            env_vars.push(("TRACE_VERBOSE".to_string(),
                           "1".to_string()));
        }

        println!("Building Tock OS in '{}'", make_work_dir.display());
        Command::new("/usr/bin/make") // assuming make is in /usr/bin
            .envs(env_vars)
            .args(&["-C", make_work_dir.to_str().unwrap()])
            .status()?;

        Ok(())
    }

    fn program(&self) -> Result<()> {
        // NOTICE: forcing use of the Hail board configuration.
        let make_work_dir = self.source_path.clone()
            .join("boards/hail");

        println!("Programming target with Tock OS from '{}'.", make_work_dir.display());
        Command::new("/usr/bin/make") // assuming make is in /usr/bin
            .args(&["-C", make_work_dir.to_str().unwrap(),
                    "program"])
            .status()?;

        Ok(())
    }
}

impl PlatformSupport for Tock {
    fn platform(&self) -> Platform {
        Platform::Tock
    }

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

    fn reconfigure(&self, trace_points: &Vec<String>) -> Result<Spec> {
        let spec = Spec::new(trace_points.iter().map(|s| s.as_ref()));

        // TODO: centralize and 'uniquify' this path.
        let spec_path = Path::new("/var/tmp/__autogen_trace.json");
        spec.write(spec_path)?;

        self.build(Some(spec_path))?;
        self.program()?;

        Ok(spec)
    }
}
