use super::Error;
use super::Loadable;
use super::Platform;

pub struct Tock {  }

impl Loadable for Tock {
    fn load(&self, path: &Path) -> Result<()> {
        Ok(())
    }

    fn unload(&self) -> Result<()> {
        Ok(())
    }

    fn platform(&self) -> Platform {
        Platform::Tock
    }
}
