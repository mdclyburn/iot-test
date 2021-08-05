//! Types and facilities common for defining tests and testbeds.

#![deny(missing_docs)]

pub mod comm;
pub mod criteria;
pub mod error;
pub mod facility;
pub mod hw;
pub mod io;
pub mod sw;
pub mod test;
pub mod trace;

type Error = error::Error;
