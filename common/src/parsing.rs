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

/// Bit-level parsing input.
pub(crate) type BitsInput<'a> = (&'a [u8], usize);

/// Bit-level parsing error over a `u8` slice.
pub(crate) type BitsResult<'a, O> = nom::IResult<(&'a [u8], usize),
                                      O,
                                      nom::error::Error<(&'a [u8], usize)>>;

/// Byte-level parsing result.
pub(crate) type ByteResult<'a, O> =
    nom::IResult<&'a [u8], O, nom::error::Error<&'a [u8]>>;

/// Byte-level parsing error over a `u8` slice.
pub(crate) type ByteError<'a> = nom::error::Error<&'a [u8]>;

/// Parse a u32 from a little-endian representation in bytes.
pub fn little_u32(b: &[u8]) -> u32 {
    let mut x: u32 = 0;
    for i in 0..4 {
        let i = 3 - i;
        x |= (b[i] as u32) << (8 * i);
    }

    x
}

/// Parse a u64 from a little-endian representation in bytes.
pub fn little_u64(b: &[u8]) -> u64 {
    let mut x: u64 = 0;
    for i in 0..8 {
        let i = 7 - i;
        x |= (b[i] as u64) << (8 * i);
    }

    x
}
