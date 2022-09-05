use std::cmp::Ordering;
use std::io::{self, Error, ErrorKind};

pub enum Received {
    None,
    Complete(Vec<u8>),
    Pending(Vec<u8>),
    Error(io::Error),
}

pub struct Message {
    pub from: u32,
    pub id: u32,
    pub size: u32,
    pub data: Vec<u8>,
}

impl Message {
    pub fn from_protocol(mut data: Vec<u8>) -> io::Result<Message> {
        if data.len() < 6 {
            return Err(smaller_size_than_protocol());
        }

        if data.len() > 65535 {
            return Err(bigger_size_than_protocol());
        }

        let from = get_u32(&data[0..2]);
        let id = get_u32(&data[2..4]);
        let size = get_u32(&data[4..6]);
        data.drain(0..6);

        Ok(Message {
            from,
            id,
            size,
            data,
        })
    }
}

pub struct Messages {
    buffer: Vec<u8>,
}

impl Messages {
    pub fn new() -> Messages {
        Messages { buffer: Vec::new() }
    }

    /// Appends the data acting like a buffer to return complete messages
    /// assumming is part of the protocol. You need to call this function in a
    /// loop and retry when Received::Pending is returned.
    pub fn feed(&mut self, mut data: Vec<u8>) -> Received {
        self.buffer.append(&mut data);
        let buffer_len = self.buffer.len() as u32;

        if buffer_len < 6 {
            self.buffer.clear();
            return Received::Error(smaller_size_than_protocol());
        }

        if buffer_len > 65535 {
            self.buffer.clear();
            return Received::Error(bigger_size_than_protocol());
        }

        // The bytes representing the message size.
        let size = get_u32(&self.buffer[4..6]);
        match size.cmp(&buffer_len) {
            Ordering::Equal => {
                // The message is complete, just send it and break.

                let result = self.buffer.to_owned();
                self.buffer.clear();

                Received::Complete(result)
            }

            Ordering::Less => {
                // The message received contains more than one message.
                // Let's split, send the first part and deal with the
                // rest on the next iteration.

                let split = self.buffer.split_off(size as usize);
                let result = self.buffer.to_owned();
                self.buffer = split;

                Received::Pending(result)
            }

            Ordering::Greater => {
                // The loop should only happen when we need to unpack
                // more than one message received in the same read, else
                // break to deal with the buffer or new messages.

                Received::None
            }
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

fn smaller_size_than_protocol() -> io::Error {
    Error::new(
        ErrorKind::Unsupported,
        "Message received is smaller than 6 bytes and thats the size of the protocol.",
    )
}

fn bigger_size_than_protocol() -> io::Error {
    Error::new(
        ErrorKind::Unsupported,
        "Message received is bigger than 65535 bytes and the protocol uses only 2 bytes to represent the size.",
    )
}
