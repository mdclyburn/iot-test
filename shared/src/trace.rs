//! Structured trace messaging.

use core::convert::From;

use crate::serialize::serialize_u32 as ser_u32;

/// Trace message carrying new system state information.
#[derive(Copy, Clone, Debug)]
pub enum TraceData {
    /// Number of active processes
    ActiveProcesses(u32),
    /// Chip is entering a low-power state.
    ChipSleep,
}

impl TraceData {
    pub fn serialize(&self, buffer: &mut [u8]) -> usize {
        buffer[0] = u8::from(self);
        let mut written = 1;
        let buffer = &mut buffer[1..];

        use TraceData::*;
        written + match *self {
            ActiveProcesses(no_procs) => ser_u32(no_procs, buffer),
            // No extra bytes written.
            _ => 0,
        }
    }
}

impl From<&TraceData> for u8 {
    fn from(counter: &TraceData) -> u8 {
        use TraceData::*;
        match counter {
            ActiveProcesses(_) => 1,
            ChipSleep => 2,
        }
    }
}
