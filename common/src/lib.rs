//! Types and facilities common for defining tests and testbeds.

#![deny(missing_docs)]

pub mod comm;
pub mod criteria;
pub mod error;
pub mod facility;
pub mod hw;
pub mod input;
pub mod io;
pub mod mem;
pub mod sw;
pub mod test;
pub mod trace;

type Error = error::Error;
