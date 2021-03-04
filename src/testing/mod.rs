/*! Defining and executing tests and evaluating their results.
 */

mod error;
mod evaluation;
mod test;
mod testbed;

pub use error::Error;
pub use evaluation::Evaluation;
pub use testbed::Testbed;
pub use test::{
    Criterion,
    Operation,
    Test,
    Execution,
    Response,
};

type Result<T> = std::result::Result<T, Error>;
