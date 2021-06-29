//! Multi-platform support interfaces.

use std::cell::RefCell;
use std::collections::HashSet;
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use super::application::Application;
use super::error::Error;
use super::instrument::Spec;
use super::Platform;
use super::PlatformSupport;
use super::Result;

/// Testbed support for the Tock OS platform.
#[derive(Clone, Debug)]
pub struct Tock {
    board: String,
    tockloader_path: PathBuf,
    // The use of this type (and the RefCell) to wrap this type is in lieu of
    // doing something more robust such as querying the device itself for its
    // software.
    loaded_apps: RefCell<HashSet<String>>,
    enabled_trace_points: RefCell<HashSet<String>>,
    source_path: PathBuf,
    app_path: PathBuf,
}

impl Tock {
    /// Create a new Tock platform instance.
    pub fn new(board: &str,
               tockloader_path: &Path,
               source_path: &Path,
               app_path: &Path) -> Tock {
        Tock {
            board: board.to_string(),
            tockloader_path: tockloader_path.to_path_buf(),
            loaded_apps: RefCell::new(HashSet::new()),
            enabled_trace_points: RefCell::new(HashSet::new()),
            source_path: source_path.to_path_buf(),
            app_path: app_path.to_path_buf(),
        }
    }

    /// Touch files containing the listed trace points to get `make` to rebuild them.
    fn touch_source<'a, T>(&self, trace_points: T) -> Result<()>
    where
        T: IntoIterator<Item = &'a String>
    {
        let kernel_path = self.source_path.clone().join("kernel/src");
        let capsules_path = self.source_path.clone().join("capsules/src");
        for trace_point_name in trace_points {
            // Find file with the trace point.
            let grep_output = Command::new("/usr/bin/grep")
                .args(&["-l",
                        "-r",
                        &trace_point_name,
                        kernel_path.to_str().unwrap(),
                        capsules_path.to_str().unwrap()])
                .output()
                .map(|output| String::from_utf8(output.stdout).unwrap().trim().to_string())?;

            for path in grep_output.lines() {
                println!("Touching '{}'.", path);
                Command::new("/usr/bin/touch")
                    .args(&[path])
                    .output()?;
            }
        }

        Ok(())
    }

    /// Retrieve a `make` command.
    fn make_command(&self) -> Command {
        // NOTICE: forcing use of the Hail board configuration.
        let make_work_dir = self.source_path.clone()
            .join("boards/hail");

        // Assuming make is in /usr/bin.
        let mut command = Command::new("/usr/bin/make");
        command
            .args(&["-C", make_work_dir.to_str().unwrap()])
            .envs(env::vars());

        command
    }

    /// Issue a `make clean`.
    #[allow(dead_code)]
    fn clean(&self) -> Result<Output> {
        let mut command = self.make_command();
        command.args(&["clean"]);

        println!("Cleaning Tock OS build with [[ {:?} ]].", command);
        command
            .output()
            .map_err(|io_err| Error::IO(io_err))
    }

    /// Build Tock OS.
    #[allow(dead_code)]
    fn build(&self) -> Result<Output> {
        let mut command = self.make_command();

        println!("Building Tock OS with [[ {:?} ]].", command);
        command
            .output()
            .map_err(|io_err| Error::IO(io_err))
    }


    /// Build Tock OS according to a spec.
    #[allow(dead_code)]
    fn build_instrumented(&self, spec: &Spec) -> Result<Output> {
        // TODO: centralize and 'uniquify' this path.
        let spec_path = Path::new("/var/tmp/__autogen_trace.json");
        spec.write(spec_path)?;

        let mut command = self.make_command();
        command.envs(vec![("TRACE_SPEC_PATH".to_string(), spec_path.to_str().unwrap().to_string()),
                          ("TRACE_VERBOSE".to_string(), "1".to_string())]);

        println!("Building instrumented Tock OS with [[ {:?} ]].", command);
        command
            .output()
            .map_err(|io_err| Error::IO(io_err))
    }

    fn program(&self) -> Result<Output> {
        let mut command = self.make_command();
        command.args(&["program"]);

        println!("Programming target with Tock OS with [[ {:?} ]].", command);
        command
            .output()
            .map_err(|io_err| Error::IO(io_err))
    }
}

impl PlatformSupport for Tock {
    fn platform(&self) -> Platform {
        Platform::Tock
    }

    fn load(&self, name: &str) -> Result<()> {
        let tockloader_path_str = self.tockloader_path.to_str().unwrap();
        let tab_name = format!("{}.tab", name);
        let app_path = self.app_path.join(tab_name);

        if !app_path.is_file() {
            return Err(Error::AppForPlatform(name.to_string(), self.platform()));
        }

        let output = Command::new(tockloader_path_str)
            .args(&["install", app_path.to_str().unwrap()])
            .output()?;

        if output.status.success() {
            self.loaded_apps.borrow_mut()
                .insert(name.to_string());
            Ok(())
        } else {
            Err(Error::Tool(output))
        }
    }

    fn unload(&self, name: &str) -> Result<()> {
        // No need to remove what's not there.
        let was_present = self.loaded_apps.borrow_mut().remove(name);
        if !was_present {
            Ok(())
        } else {
            let tockloader_path_str = self.tockloader_path.to_str().unwrap();

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

    fn loaded_software(&self) -> HashSet<String> {
        self.loaded_apps.borrow().iter()
            .cloned()
            .collect()
    }

    fn reconfigure(&self, trace_points: &Vec<String>) -> Result<Spec> {
        // Do not rebuild if the desired points are already enabled.
        let trace_points: HashSet<String> = trace_points.into_iter()
            .cloned()
            .collect();
        let already_enabled = self.enabled_trace_points.borrow()
            .is_superset(&trace_points);
        if !already_enabled {
            println!("Triggering rebuild of Tock. Need new trace points enabled.");
            self.touch_source(&trace_points)?;

            let spec = Spec::new(trace_points.iter().map(|s| s.as_ref()));
            // let output = self.build_instrumented(&spec)?;
            // let stdout = String::from_utf8(output.stdout.clone())
            //     .unwrap_or("<<Could not process stdout output.>>".to_string());
            // let stderr = String::from_utf8(output.stderr.clone())
            //     .unwrap_or("<<Could not process stderr output.>>".to_string());
            // println!(">>>>>>>>>>>>>>>> STDOUT:\n{}\n\n>>>>>>>>>>>>> STDERR:\n{}", stdout, stderr);

            // if !output.status.success() {
            //     Err(Error::Tool(output))
            // } else {
            //     self.program()?;
            //     Ok(spec)
            // }
            Ok(spec)
        } else {
            println!("Using currently deployed build of Tock.");
            Ok(Spec::new(self.enabled_trace_points.borrow().iter().map(|s| s.as_ref())))
        }
    }
}
