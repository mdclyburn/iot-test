//! Parsing helper types and functions.

use nom;
use nom::{
    bits::bytes as make_bit_compatible,
    bits::streaming as bits,
    bytes::streaming as bytes,

    branch,
    combinator,
    sequence,
};

pub(crate) type BitsInput<'a> = (&'a [u8], usize);
pub(crate) type BitsResult<'a, O> = nom::IResult<(&'a [u8], usize),
                                      O,
                                      nom::error::Error<(&'a [u8], usize)>>;
pub(crate) type ByteError<'a> = nom::error::Error<&'a [u8]>;

macro_rules! little_u32 {
    ($b0:expr, $b8:expr, $b16:expr, $b24:expr) => {{
        let val = ((($b0) as u32) << 0
                   | (($b8) as u32) << 8
                   | (($b16) as u32) << 16
                   | (($b24) as u32) << 24);
        val
    }}
}

// For usages in `mem` module.
pub(crate) use little_u32;
