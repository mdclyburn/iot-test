//! Aggregate memory statistics sent over the wire.

use std::convert::TryFrom;

fn extract_u32<I>(data: &mut I) -> Result<u32, String>
where
    I: Iterator<Item = u8>
{
    let (b0, b1, b2, b3) = (
        data.next().ok_or("u32 ended at byte 0".to_string())?,
        data.next().ok_or("u32 ended at byte 1".to_string())?,
        data.next().ok_or("u32 ended at byte 2".to_string())?,
        data.next().ok_or("u32 ended at byte 3".to_string())?);

    Ok(b0 as u32 | (b1 as u32) << 8 | (b2 as u32) << 16 | (b3 as u32) << 24)
}

pub mod hil {
    //! Memory counter information to be used by the device under test.
    use core::convert::TryFrom;
    use core::fmt::{self, Display};

    /// Memory statistic category
    #[derive(Copy, Clone, Eq, PartialEq)]
    pub enum CounterId {
        /// Total for allocated grant types.
        AllGrantStructures(u32),
        /// Custom grant allocation total.
        CustomGrant(u32),
        /// Sizes of individual grants.
        Grant(u32, u32),
        /// Grant pointer table.
        GrantPointerTable(u32),
        /// Process control block.
        PCB(u32),
        /// Upcall queue.
        UpcallQueue(u32),
    }

    impl Display for CounterId {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            use CounterId::*;
            match self {
                AllGrantStructures(pid) => write!(f, "grant total for {}", pid),
                CustomGrant(pid) => write!(f, "custom grant total for {}", pid),
                Grant(pid, grant_no) => write!(f, "grant #{} for process {}", grant_no, pid),
                GrantPointerTable(pid) => write!(f, "grant pointer table for process {}", pid),
                PCB(pid) => write!(f, "PCB for process {}", pid),
                UpcallQueue(pid) => write!(f, "upcall queue for process {}", pid),
            }
        }
    }

    impl TryFrom<(u8, &[u8])> for CounterId {
        type Error = String;

        fn try_from(val: (u8, &[u8])) -> Result<CounterId, String> {
            let (counter_val, specifics) = val;
            let mut stream_it = specifics.iter().copied();

            use CounterId::*;
            use super::extract_u32;
            Ok(match counter_val {
                1 => PCB(extract_u32(&mut stream_it)?),
                2 => UpcallQueue(extract_u32(&mut stream_it)?),
                3 => GrantPointerTable(extract_u32(&mut stream_it)?),
                4 => Grant(extract_u32(&mut stream_it)?,
                           extract_u32(&mut stream_it)?),
                5 => CustomGrant(extract_u32(&mut stream_it)?),

                _ => return Err(format!("counter not identified: {}", counter_val)),
            })
        }
    }
}

use self::hil::*;

/// Operation to apply to aggregated memory statistic.
pub enum StreamOperation {
    /// Add the contained value to the given statistic's counter.
    Add(CounterId, u32),
    /// Set the counter for the given statistic to the given value.
    Set(CounterId, u32),
}

impl TryFrom<&[u8]> for StreamOperation {
    type Error = String;

    fn try_from(val: &[u8]) -> Result<StreamOperation, String> {
        let mut data_it = val.iter().copied();

        // operation/counter ID
        let op_counter = data_it.next().ok_or(
            "no aggregate op-counter byte".to_string())?;
        let op_val: u8 = (op_counter & 0b1000_0000) >> 7;
        let counter_val: u8 = op_counter & 0b0111_1111;

        let counter_data = &val[1..val.len().saturating_sub(4)];
        let counter = CounterId::try_from((counter_val, counter_data))?;

        use StreamOperation::*;
        Ok(match op_val {
            0 => Set(counter, extract_u32(&mut data_it)?),
            1 => Add(counter, extract_u32(&mut data_it)?),

            _ => return Err(format!("invalid operation value: {}", op_val)),
        })
    }
}
