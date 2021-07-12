use std::fmt::Debug;
use std::iter::Iterator;

use crate::testing::testbed::Testbed;
use crate::testing::test::Test;

pub mod error;
pub mod hard_code;
pub mod json;

type Result<T> = std::result::Result<T, error::Error>;

/// Configuration reader producing a configured testbed from an input source.
pub trait TestbedConfigReader: Debug {
    /// Create a configured testbed ready to run tests.
    fn create(&self) -> Result<Testbed>;
}

/// Data adapter producing tests read from an input source.
pub trait TestConfigAdapter: Iterator<Item = Result<Test>> {  }
