use std::iter::Iterator;

use crate::testing::testbed::Testbed;
use crate::testing::test::Test;

mod error;
mod json;

type Result<T> = std::result::Result<T, error::Error>;

/// Configuration reader producing a configured testbed from an input source.
pub trait TestbedConfigReader {
    /// Create a configured testbed ready to run tests.
    fn create(&self) -> Result<Testbed>;
}

/// Data adapter producing tests read from an input source.
pub trait TestConfigAdapter: Iterator<Item = Result<Test>> {  }
