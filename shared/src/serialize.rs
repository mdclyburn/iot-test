//! Data serialization helpers.

/// Place a 32-bit unsigned integer into a buffer.
pub fn serialize_u32(n: u32, buffer: &mut [u8]) -> usize {
    buffer[0] = (n & 0xFF) as u8;
    buffer[1] = ((n >> 8) & 0xFF) as u8;
    buffer[2] = ((n >> 16) & 0xFF) as u8;
    buffer[3] = ((n >> 24) & 0xFF) as u8;

    4
}
