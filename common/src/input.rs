//! Providers for tests.

use std::fmt::Debug;

use crate::test::Test;

/// Data adapter producing tests read from an input source.
pub trait TestProvider: Debug {
    /// Create a Test-producing iterator.
    fn tests<'a>(&'a self) -> Box<dyn Iterator<Item = Test> + 'a>;
}
