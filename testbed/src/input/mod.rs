//! Pluggable testbed and test providers.

use std::fmt::Debug;

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
