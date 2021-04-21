use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::Error;
use super::Platform;
use super::Result;

/// Collection of the same (or similar) applications for different platforms.
pub struct ApplicationSet {
    id: String,
    app_set: HashMap<Platform, PathBuf>,
}

impl ApplicationSet {
    /// Create a new Application set.
    pub fn new<'a, T>(id: &str, files: T) -> ApplicationSet
    where
        T: IntoIterator<Item = (Platform, &'a Path)>
    {
        ApplicationSet {
            id: id.to_string(),
            app_set: files.into_iter()
                .map(|(platform, path)| (platform, path.to_path_buf()))
                .collect(),
        }
    }

    /// Get the path to the application for the given platform.
    pub fn get_for(&self, platform: Platform) -> Result<&Path> {
        self.app_set.get(&platform)
            .map(|p| p.as_path())
            .ok_or(Error::UndefinedApp(self.id.clone(), platform))
    }
}
