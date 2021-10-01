//! Aggregate memory statistics sent over the wire.

use std::time::Instant;
use std::fmt::{self, Display};

use nom::{
    bits::bytes as make_bit_compatible,
    bits::streaming as bits,
    bytes::streaming as bytes,

    branch,
    combinator,
    sequence,
};
use flexbed_shared::mem::CounterId;

/// Operation to apply to aggregated memory statistic.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum StreamOperation {
    /// Add to a statistic counter.
    Add,
    /// Set a statistic counter to a value.
    Set,
}

/// Memory counter update event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryTrace {
    time: Instant,
    op: StreamOperation,
    counter: CounterId,
    value: u32,
}

impl MemoryTrace {
    /// When the event occurred.
    pub fn time(&self) -> Instant {
        self.time
    }

    /// How the trace event changes the counter.
    pub fn operation(&self) -> StreamOperation {
        self.op
    }

    /// Counter identification data.
    pub fn counter(&self) -> &CounterId {
        &self.counter
    }

    /// Counter value.
    pub fn value(&self) -> u32 {
        self.value
    }
}

impl Display for MemoryTrace {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "operation: {:?}, counter: {:35}, value: {}",
               self.op, self.counter, self.value)
    }
}

type BitsInput<'a> = (&'a [u8], usize);
type BitsResult<'a, O> =
    nom::IResult<(&'a [u8], usize),
                 O,
                 nom::error::Error<(&'a [u8], usize)>>;
type ByteError<'a> = nom::error::Error<&'a [u8]>;

fn stream_operation_op<'a>(input: BitsInput<'a>) -> BitsResult<StreamOperation> {
    branch::alt(
        (combinator::value(StreamOperation::Add, bits::tag(0usize, 1usize)),
         (combinator::value(StreamOperation::Set, bits::tag(1usize, 1usize)))))
        (input)
}

fn counter_stream<'a>(id: usize,
                      specific_byte_len: usize,
                      construct: impl Fn(&'a [u8]) -> CounterId) -> impl FnMut(BitsInput<'a>) -> BitsResult<CounterId>
{
    let f_get_data = sequence::preceded(
        bits::tag(id, 7usize),
        make_bit_compatible::<&[u8], _, ByteError<'a>, _, _>(bytes::take(specific_byte_len)));
    combinator::map(f_get_data, construct)
}

macro_rules! little_u32 {
    ($b0:expr, $b8:expr, $b16:expr, $b24:expr) => {{
        let val = ((($b0) as u32) << 0
                   | (($b8) as u32) << 8
                   | (($b16) as u32) << 16
                   | (($b24) as u32) << 24);
        val
    }}
}

fn pcb(input: BitsInput) -> BitsResult<CounterId> {
    counter_stream(1, 4, |s: &[u8]| {
        CounterId::PCB(little_u32!(s[0], s[1], s[2], s[3]))
    })(input)
}

fn upcall_queue(input: BitsInput) -> BitsResult<CounterId> {
    counter_stream(2, 4, |s: &[u8]| {
        CounterId::UpcallQueue(little_u32!(s[0], s[1], s[2], s[3]))
    })(input)
}

fn grant_pointer_table(input: BitsInput) -> BitsResult<CounterId> {
    counter_stream(3, 4, |s: &[u8]| {
        CounterId::GrantPointerTable(little_u32!(s[0], s[1], s[2], s[3]))
    })(input)
}

fn grant(input: BitsInput) -> BitsResult<CounterId> {
    counter_stream(4, 8, |s: &[u8]| {
        CounterId::Grant(
            little_u32!(s[0], s[1], s[2], s[3]),
            little_u32!(s[4], s[5], s[6], s[7]))
    })(input)
}

fn custom_grant(input: BitsInput) -> BitsResult<CounterId> {
    counter_stream(5, 4, |s: &[u8]| {
        CounterId::CustomGrant(little_u32!(s[0], s[1], s[2], s[3]))
    })(input)
}

fn counter(input: BitsInput) -> BitsResult<CounterId> {
    branch::alt((pcb, upcall_queue, grant_pointer_table, grant, custom_grant))
        (input)
}

fn streamed_counter<'a>(input: BitsInput<'a>, time: Instant) -> BitsResult<MemoryTrace> {
    // Read the stream operation, the counter data, and the u32 at the end.
    let streamed_delta = sequence::tuple(
        (stream_operation_op,
         counter,
         make_bit_compatible::<&[u8], _, ByteError<'a>, _, _>(bytes::take(4usize))));

    // Build the final StreamOperation value.
    combinator::map(streamed_delta, |(op, counter, vb)| {
        MemoryTrace {
            time,
            op: op,
            counter: counter,
            value: little_u32!(vb[0], vb[1], vb[2], vb[3]),
        }
    })
        (input)
}

/// Recreate stream operation from raw bytes.
///
/// Parses the provided sequence of bytes and returns a structured view of the stream operation.
/// If the parsing fails, then this function returns an `Err` that describes the reason for the failure (in raw `nom` terms...).
pub fn parse_counter(input: &[u8], time: Instant) -> BitsResult<MemoryTrace> {
    streamed_counter((input, 0), time)
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use super::StreamOperation;

    #[test]
    pub fn recognize_add_operation() {
        let input = [0b0000_0000];
        let r = stream_operation_op((&input, 0));
        assert_eq!(r.map(|(_i, op)| op).unwrap(),
                   StreamOperation::Add);
    }

    #[test]
    pub fn recognize_set_operation() {
        let input = [0b1000_0000];
        let r = stream_operation_op((&input, 0));
        assert_eq!(r.map(|(_i, op)| op).unwrap(),
                   StreamOperation::Set);
    }

    #[test]
    pub fn recognize_pcb() {
        let input = [0b0000_0001 << 1,
                     0b0001_0000 << 1,
                     0b0000_1000 << 1,
                     0b0000_0010 << 1,
                     0b0000_0001 << 1];
        let r = pcb((&input, 0));

        assert_eq!(r.is_ok(), true);
        assert_eq!(r.map(|(_i, c)| c).unwrap(),
                   CounterId::PCB((0b0001_0000 as u32) << (1)
                                  | (0b0000_1000 as u32) << (1 + 8)
                                  | (0b0000_0010 as u32) << (1 + 16)
                                  | (0b0000_0001 as u32) << (1 + 24)));
    }

    #[test]
    pub fn recognize_upcall_queue() {
        let input = [2 << 1,
                     0b0001_0000 << 1,
                     0b0000_1000 << 1,
                     0b0000_0010 << 1,
                     0b0000_0001 << 1];
        let r = upcall_queue((&input, 0));

        assert_eq!(r.is_ok(), true);
        assert_eq!(r.map(|(_i, c)| c).unwrap(),
                   CounterId::UpcallQueue((0b0001_0000 as u32) << (1)
                                          | (0b0000_1000 as u32) << (1 + 8)
                                          | (0b0000_0010 as u32) << (1 + 16)
                                          | (0b0000_0001 as u32) << (1 + 24)));
    }

    #[test]
    pub fn recognize_grant_pointer_table() {
        let input = [3 << 1,
                     0b0001_0000 << 1,
                     0b0000_1000 << 1,
                     0b0000_0010 << 1,
                     0b0000_0001 << 1];
        let r = grant_pointer_table((&input, 0));

        assert_eq!(r.is_ok(), true);
        assert_eq!(r.map(|(_i, c)| c).unwrap(),
                   CounterId::GrantPointerTable((0b0001_0000 as u32) << (1)
                                                | (0b0000_1000 as u32) << (1 + 8)
                                                | (0b0000_0010 as u32) << (1 + 16)
                                                | (0b0000_0001 as u32) << (1 + 24)));
    }

    #[test]
    pub fn recognize_grant() {
        let input = [4 << 1,
                     0b0001_0000 << 1,
                     0b0000_1000 << 1,
                     0b0000_0010 << 1,
                     0b0000_0001 << 1,
                     0b0001_0000 << 2,
                     0b0000_1000 << 2,
                     0b0000_0010 << 2,
                     0b0000_0001 << 2];
        let r = grant((&input, 0));
        assert_eq!(r.map(|(_i, c)| c).unwrap(),
                   CounterId::Grant(
                       (0b0001_0000 as u32) << (1)
                           | (0b0000_1000 as u32) << (1 + 8)
                           | (0b0000_0010 as u32) << (1 + 16)
                           | (0b0000_0001 as u32) << (1 + 24),
                       (0b0001_0000 as u32) << (2)
                           | (0b0000_1000 as u32) << (2 + 8)
                           | (0b0000_0010 as u32) << (2 + 16)
                           | (0b0000_0001 as u32) << (2 + 24)));
    }

    #[test]
    pub fn recognize_custom_grant() {
        let input = [5 << 1,
                     0b0001_0000 << 1,
                     0b0000_1000 << 1,
                     0b0000_0010 << 1,
                     0b0000_0001 << 1];
        let r = custom_grant((&input, 0));

        assert_eq!(r.is_ok(), true);
        assert_eq!(r.map(|(_i, c)| c).unwrap(),
                   CounterId::CustomGrant((0b0001_0000 as u32) << (1)
                                          | (0b0000_1000 as u32) << (1 + 8)
                                          | (0b0000_0010 as u32) << (1 + 16)
                                          | (0b0000_0001 as u32) << (1 + 24)));
    }

    #[test]
    pub fn streamed_pcb_counter() {
        let input = [0b1000_0001,

                     0b0000_0110,
                     0b0000_0000,
                     0b0000_0000,
                     0b0000_0000];
        let now = Instant::now();
        let r = streamed_counter((&input, 0), now);

        assert!(r.is_ok());
        assert_eq!(
            r.map(|(_i, c)| c).unwrap(),
            MemoryTrace {
                time: now,
                op: StreamOperation::Set,
                counter: CounterId::PCB(6)
            });
    }

    #[test]
    pub fn incomplete_counter() {
        let input = [0b1000_0001,

                     0b0000_0110];

        let now = Instant::now();

        let r = streamed_counter((&input, 0), now);
        println!("counter: {:?}", r);

        assert!(r.is_ok());
    }
}
