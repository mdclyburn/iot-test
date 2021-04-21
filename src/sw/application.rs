use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::Error;
use super::Platform;
use super::Result;

/// Collection of the same (or similar) applications for different platforms.
#[derive(Clone, Debug)]
pub struct Application {
    id: String,
    app_set: HashMap<Platform, PathBuf>,
}

impl Application {
    /// Create a new Application.
    pub fn new<'a, T>(id: &str, files: T) -> Application
    where
        T: IntoIterator<Item = &'a (Platform, &'a Path)>
    {
        Application {
            id: id.to_string(),
            app_set: files.into_iter()
                .map(|(platform, path)| (*platform, path.to_path_buf()))
                .collect(),
        }
    }

    /// Returns the shorthand identifier for the application.
    pub fn get_id(&self) -> &str {
        &self.id
    }

    /// Get the path to the application for the given platform.
    pub fn get_for(&self, platform: Platform) -> Result<&Path> {
        self.app_set.get(&platform)
            .map(|p| p.as_path())
            .ok_or(Error::UndefinedApp(self.id.clone(), platform))
    }
}

#[derive(Debug)]
pub struct ApplicationSet {
    applications: HashMap<String, Application>,
}

impl ApplicationSet {
    /// Create a new application set.
    pub fn new<'a, T>(apps: T) -> ApplicationSet
    where
        T: IntoIterator<Item = &'a Application>
    {
        ApplicationSet {
            applications: apps.into_iter()
                .cloned()
                .map(|app| (app.get_id().to_string(), app))
                .collect(),
        }
    }
}
