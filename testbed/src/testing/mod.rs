/*! Defining and executing tests and evaluating their results.
 */

pub mod error;
pub mod evaluation;
pub mod testbed;

use error::Error;

type Result<T> = std::result::Result<T, Error>;