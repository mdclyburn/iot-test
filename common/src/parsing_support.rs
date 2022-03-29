//! Parsing helper types and functions.

use nom;
use nom::{
    bytes,
    combinator,
};

/// Bit-level parsing input.
pub type BitsInput<'a> = (&'a [u8], usize);

/// Bit-level parsing error over a `u8` slice.
pub type BitsResult<'a, O> = nom::IResult<(&'a [u8], usize),
                                      O,
                                      nom::error::Error<(&'a [u8], usize)>>;

/// Byte-level parsing result.
pub type ByteResult<'a, O> =
    nom::IResult<&'a [u8], O, nom::error::Error<&'a [u8]>>;

/// Byte-level parsing error over a `u8` slice.
pub type ByteError<'a> = nom::error::Error<&'a [u8]>;

/// Bit-level parsing error over a `u8` slice.
pub type BitError<'a> = nom::error::Error<(&'a [u8], usize)>;

/// Parse a u32 from a little-endian representation in bytes.
pub fn little_u32<'a>(data: &'a [u8]) -> ByteResult<'a, u32> {
    combinator::map(
        bytes::complete::take(4usize),
        |b: &[u8]| {
            let mut x: u32 = 0;
            for i in 0..4 {
                let i = 3 - i;
                x |= (b[i] as u32) << (8 * i);
            }

            x
        })
        (data)
}

/// Parse a u64 from a little-endian representation in bytes.
pub fn little_u64<'a>(data: &'a [u8]) -> ByteResult<'a, u64> {
    combinator::map(
        bytes::complete::take(8usize),
        |b: &[u8]| {
            let mut x: u64 = 0;
            for i in 0..8 {
                let i = 7 - i;
                x |= (b[i] as u64) << (8 * i);
            }

            x
        })
        (data)
}
