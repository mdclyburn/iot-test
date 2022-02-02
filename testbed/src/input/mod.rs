//! Pluggable testbed and test providers.

use std::fmt::Debug;

use crate::testing::testbed::Testbed;

pub mod error;
pub mod hard_code;
pub mod shared_lib;

type Result<T> = std::result::Result<T, error::Error>;

/// Adapter producing a testbed from some input source.
pub trait TestbedProvider: Debug {
    /// Create a configured testbed ready to run tests.
    fn create(&self) -> Result<Testbed>;
}
