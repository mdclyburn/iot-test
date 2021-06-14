/*! Defining and executing tests and evaluating their results.
 */

pub mod criteria;
pub mod error;
pub mod evaluation;
pub mod test;
pub mod testbed;
pub mod trace;

use error::Error;

type Result<T> = std::result::Result<T, Error>;
