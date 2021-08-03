//! Pluggable testbed and test providers.

use std::fmt::Debug;
use std::iter::Iterator;

use flexbed_common::test::Test;

use crate::testing::testbed::Testbed;

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
pub trait TestConfigAdapter: Debug {
    /// Create a Test-producing iterator.
    fn tests(&self) -> Box<dyn Iterator<Item = Test> + '_>;
}
