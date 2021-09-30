//! Aggregate memory statistics sent over the wire.

use nom::{
    bits::bytes as make_bit_compatible,
    bits::complete as bits,
    bytes::complete as bytes,

    branch,
    combinator,
    multi,
    sequence,
};
use flexbed_shared::mem::CounterId;

/// Operation to apply to aggregated memory statistic.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum StreamOperation {
    /// Add the contained value to the given statistic's counter.
    Add(CounterId, u32),
    /// Set the counter for the given statistic to the given value.
    Set(CounterId, u32),
}

type BitsInput<'a> = (&'a [u8], usize);
type BitsResult<'a, O> =
    nom::IResult<(&'a [u8], usize),
                 O,
                 nom::error::Error<(&'a [u8], usize)>>;
type ByteError<'a> = nom::error::Error<&'a [u8]>;

#[derive(Clone, Debug, Eq, PartialEq)]
enum OpType { Add, Set }

fn stream_operation<'a>(input: BitsInput<'a>) -> BitsResult<OpType> {
    branch::alt(
        (combinator::value(OpType::Add, bits::tag(0usize, 1usize)),
         (combinator::value(OpType::Set, bits::tag(1usize, 1usize)))))
        (input)
}

fn parse_little_u32<'a>(input: BitsInput<'a>) -> BitsResult<u32> {
    type ByteError<'a> = nom::error::Error<&'a [u8]>;
    combinator::map(
        make_bit_compatible::<&'a [u8], _, ByteError<'a>, _, _>(bytes::take(4usize)),
        |s: &[u8]| {
            (s[0] as u32) << 0
                | (s[1] as u32) <<  8
                | (s[2] as u32) << 16
                | (s[3] as u32) << 24
        })
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

fn streamed_counter(input: BitsInput) -> BitsResult<StreamOperation> {
    // Read the stream operation, the counter data, and the u32 at the end.
    let streamed_delta = sequence::tuple(
        (stream_operation, counter, parse_little_u32));

    // Build the final StreamOperation value.
    combinator::map(streamed_delta, |(op, counter, d)| {
        match op {
            OpType::Add => StreamOperation::Add(counter, d),
            OpType::Set => StreamOperation::Set(counter, d),
        }
    })
        (input)
}


/// Recreate a sequence of stream operations from raw bytes.
///
/// Parses the provided sequence of bytes and returns a structured view of the streamed data.
/// If the parsing fails, then this function returns an `Err` that describes the reason for the failure (in raw `nom` terms...).
pub fn parse_stream(input: &[u8]) -> Result<Vec<StreamOperation>, String> {
    let input = (input, 0);
    multi::many0(streamed_counter)(input)
        .map(|(_input, ops)| ops)
        .map_err(|e| format!("Memory stat stream parsing failed.\nNom error: {}", e))
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use super::parse::OpType;

    #[test]
    pub fn recognize_add_operation() {
        let input = [0b0000_0000];
        let r = parse::stream_operation((&input, 0));
        assert_eq!(r.map(|(_i, op)| op).unwrap(),
                   OpType::Add);
    }

    #[test]
    pub fn recognize_set_operation() {
        let input = [0b1000_0000];
        let r = parse::stream_operation((&input, 0));
        assert_eq!(r.map(|(_i, op)| op).unwrap(),
                   OpType::Set);
    }

    #[test]
    pub fn recognize_pcb() {
        let input = [0b0000_0001 << 1,
                     0b0001_0000 << 1,
                     0b0000_1000 << 1,
                     0b0000_0010 << 1,
                     0b0000_0001 << 1];
        let r = parse::pcb((&input, 0));

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
        let r = parse::upcall_queue((&input, 0));

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
        let r = parse::grant_pointer_table((&input, 0));

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
        let r = parse::grant((&input, 0));
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
        let r = parse::custom_grant((&input, 0));

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
                     0b0000_0000,
                     0b0000_1111,
                     0b0000_0000,
                     0b0000_0000,
                     0b0000_0000];
        let r = parse::streamed_counter((&input, 0));

        assert!(r.is_ok());
        assert_eq!(r.map(|(_i, c)| c).unwrap(),
                   StreamOperation::Set(CounterId::PCB(6), 15));
    }

    #[test]
    pub fn stream() {
        let input = [0b1000_0100,
                     0b0000_0101,
                     0b0000_0000,
                     0b0000_0000,
                     0b0000_0000,
                     0b0000_0000, // unused 4 bytes
                     0b0000_0000,
                     0b0000_0000,
                     0b0000_0000,
                     0b0010_0000,
                     0b0000_0000,
                     0b0000_0000,
                     0b0000_0000,

                     0b0000_0100,
                     0b0000_0101,
                     0b0000_0000,
                     0b0000_0000,
                     0b0000_0000,
                     0b0000_0000, // unused 4 bytes
                     0b0000_0000,
                     0b0000_0000,
                     0b0000_0000,
                     0b0001_1100,
                     0b0000_0000,
                     0b0000_0000,
                     0b0000_0000,];
        let r = parse::stream(&input);

        assert!(r.is_ok());
        assert_eq!(r.as_ref().map(|stream| stream).unwrap()[0],
                   StreamOperation::Set(CounterId::Grant(5, 0), 32));
        assert_eq!(r.map(|stream| stream).as_ref().unwrap()[1],
                   StreamOperation::Add(CounterId::Grant(5, 0), 28));
    }
}
