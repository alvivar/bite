pub struct Message {
    pub from: u32,
    pub id: u32,
    pub size: u32,
    pub data: Vec<u8>,
}

impl Message {
    pub fn from_protocol(mut data: Vec<u8>) -> Message {
        let from = get_u32(&data[0..2]);
        let id = get_u32(&data[2..4]);
        let size = get_u32(&data[4..6]);
        data.drain(0..6);

        Message {
            from,
            id,
            size,
            data,
        }
    }
}

pub fn get_u32(bytes: &[u8]) -> u32 {
    (bytes[0] as u32) << 8 | bytes[1] as u32
}

/// Protocol: Client id, message id and full size, 2 bytes eachs, from the first
/// 6 bytes of the message.
fn get_header(from: u32, id: u32, size: u32) -> [u8; 6] {
    let byte0 = ((from & 0xFF00) >> 8) as u8;
    let byte1 = (from & 0x00FF) as u8;

    let byte2 = ((id & 0xFF00) >> 8) as u8;
    let byte3 = (id & 0x00FF) as u8;

    let byte4 = ((size & 0xFF00) >> 8) as u8;
    let byte5 = (size & 0x00FF) as u8;

    [byte0, byte1, byte2, byte3, byte4, byte5]
}

pub fn stamp_header(mut bytes: Vec<u8>, from: u32, id: u32) -> Vec<u8> {
    let size = (bytes.len() + 6) as u32;
    bytes.splice(0..0, get_header(from, id, size));
    bytes
}
