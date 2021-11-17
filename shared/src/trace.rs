//! Structured trace messaging.

use core::convert::From;

use crate::serialize::{
    serialize_u32 as ser_u32,
    deserialize_u32 as deser_u32,
};

/// Trace message carrying new system state information.
#[derive(Copy, Clone, Debug)]
pub enum TraceData {
    /// Amount of work the kernel can execute on has changed.
    KernelWork(u32),
    /// A process has suspended execution.
    ProcessSuspended(u32),
    /// The kernel has gotten around to servicing an interrupt.
    InterruptServiced(u32)
}

impl TraceData {
    pub fn serialize(&self, buffer: &mut [u8]) -> usize {
        buffer[0] = u8::from(self);
        let buffer = &mut buffer[1..];

        use TraceData::*;
        1 + match *self {
            KernelWork(no_procs) => ser_u32(no_procs, buffer),
            ProcessSuspended(executed_for_us) => ser_u32(executed_for_us, buffer),
            InterruptServiced(interrupt_no) => ser_u32(interrupt_no, buffer),
        }
    }

    pub fn deserialize(buffer: &[u8]) -> Result<(TraceData, usize), ()> {
        if buffer.len() < 1 {
            Err(())
        } else {
            let header = buffer[0];
            let buffer = &buffer[1..];

            use TraceData::*;
            let (trace, consumed) = match header {
                1 => (KernelWork(deser_u32(&buffer)?), 4),
                2 => (ProcessSuspended(deser_u32(&buffer)?), 4),
                3 => (InterruptServiced(deser_u32(&buffer)?), 4),
                _ => Err(())?,
            };

            Ok((trace, 1 + consumed))
        }
    }
}

impl From<&TraceData> for u8 {
    fn from(counter: &TraceData) -> u8 {
        use TraceData::*;
        match counter {
            KernelWork(_) => 1,
            ProcessSuspended(_) => 2,
            InterruptServiced(_) => 3,
        }
    }
}
