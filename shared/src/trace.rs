//! Structured trace messaging.

use core::convert::From;

use crate::serialize::serialize_u32 as ser_u32;

/// Trace message carrying new system state information.
#[derive(Copy, Clone, Debug)]
pub enum TraceData {
    /// Number of active processes
    KernelWork(u32),
    /// A process has suspended execution.
    ProcessSuspended(u32),
}

impl TraceData {
    pub fn serialize(&self, buffer: &mut [u8]) -> usize {
        buffer[0] = u8::from(self);
        let buffer = &mut buffer[1..];

        use TraceData::*;
        1 + match *self {
            KernelWork(no_procs) => ser_u32(no_procs, buffer),
            ProcessSuspended(executed_for_us) => ser_u32(executed_for_us, buffer),
        }
    }
}

impl From<&TraceData> for u8 {
    fn from(counter: &TraceData) -> u8 {
        use TraceData::*;
        match counter {
            KernelWork(_) => 1,
            ProcessSuspended(_) => 2,
        }
    }
}
