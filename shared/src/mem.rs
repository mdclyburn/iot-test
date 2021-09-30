//! Memory stat tracking.

//! Memory counter information to be used by the device under test.

use core::convert::From;
use core::fmt::{self, Display};

/// Memory statistic category.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CounterId {
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
            CustomGrant(pid) => write!(f, "custom grant total for {}", pid),
            Grant(pid, grant_no) => write!(f, "grant #{} for process {}", grant_no, pid),
            GrantPointerTable(pid) => write!(f, "grant pointer table for process {}", pid),
            PCB(pid) => write!(f, "PCB for process {}", pid),
            UpcallQueue(pid) => write!(f, "upcall queue for process {}", pid),
        }
    }
}

impl From<CounterId> for u8 {
    fn from(counter: CounterId) -> u8 {
        match counter {
            CounterId::PCB(_) => 1,
            CounterId::UpcallQueue(_) => 2,
            CounterId::GrantPointerTable(_) => 3,
            CounterId::Grant(_, _) => 4,
            CounterId::CustomGrant(_) => 5,
        }
    }
}
