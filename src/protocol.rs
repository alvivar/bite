/// Insert at the beginning 2 bytes representing the size of the frame.
pub fn insert_size(mut bytes: Vec<u8>) -> Vec<u8> {
    let len = (bytes.len() + 2) as u64;
    let byte0 = ((len & 0xFF00) >> 8) as u8;
    let byte1 = (len & 0x00FF) as u8;
    let size = [byte0, byte1];

    bytes.splice(0..0, size);
    bytes
}

/// Returns the first two bytes as a u32 big endian, conceptually the size of
/// the frame.
pub fn get_size(bytes: &[u8]) -> u32 {
    (bytes[0] as u32) << 8 | bytes[1] as u32
}
