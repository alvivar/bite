use std::io::{self, Error, ErrorKind};

const HEADER_SIZE: usize = 6;

pub struct Message {
    pub from: u32,
    pub id: u32,
    pub size: u32,
    pub data: Vec<u8>,
}

pub struct Messages {
    buffer: Vec<u8>,
}

impl Messages {
    pub fn new() -> Messages {
        Messages { buffer: Vec::new() }
    }

    /// Appends bytes read from the socket, which may contain any part of one
    /// or more messages.
    pub fn feed(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Removes and returns the next complete message, or `None` when more
    /// bytes are needed. Fails when the header declares a size smaller than
    /// the header itself.
    pub fn next_message(&mut self) -> io::Result<Option<Message>> {
        if self.buffer.len() < HEADER_SIZE {
            return Ok(None);
        }

        let size = get_u32(&self.buffer[4..6]) as usize;

        if size < HEADER_SIZE {
            return Err(smaller_size_than_protocol());
        }

        if self.buffer.len() < size {
            return Ok(None);
        }

        let message = Message {
            from: get_u32(&self.buffer[0..2]),
            id: get_u32(&self.buffer[2..4]),
            size: size as u32,
            data: self.buffer[HEADER_SIZE..size].to_vec(),
        };
        self.buffer.drain(..size);

        Ok(Some(message))
    }

    /// True while the buffer holds part of a message.
    pub fn is_incomplete(&self) -> bool {
        !self.buffer.is_empty()
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

pub fn stamp_header(mut data: Vec<u8>, from: u32, id: u32) -> Vec<u8> {
    let size = (data.len() + 6) as u32;
    data.splice(0..0, get_header(from, id, size));
    data
}

fn smaller_size_than_protocol() -> io::Error {
    Error::new(
        ErrorKind::Unsupported,
        "Message size is smaller than 6 bytes and thats the size of the protocol header.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(from: u32, id: u32, data: &[u8]) -> Vec<u8> {
        stamp_header(data.to_vec(), from, id)
    }

    #[test]
    fn waits_for_split_header() {
        let bytes = frame(1, 2, b"hello");
        let mut messages = Messages::new();

        messages.feed(&bytes[..3]);
        assert!(messages.next_message().unwrap().is_none());
        assert!(messages.is_incomplete());

        messages.feed(&bytes[3..]);
        let message = messages.next_message().unwrap().unwrap();
        assert_eq!(message.from, 1);
        assert_eq!(message.id, 2);
        assert_eq!(message.size, 11);
        assert_eq!(message.data, b"hello");
        assert!(!messages.is_incomplete());
    }

    #[test]
    fn waits_for_split_body() {
        let bytes = frame(1, 2, b"hello");
        let mut messages = Messages::new();

        messages.feed(&bytes[..8]);
        assert!(messages.next_message().unwrap().is_none());
        assert!(messages.is_incomplete());

        messages.feed(&bytes[8..]);
        let message = messages.next_message().unwrap().unwrap();
        assert_eq!(message.data, b"hello");
        assert!(!messages.is_incomplete());
    }

    #[test]
    fn extracts_coalesced_messages_larger_than_one_frame() {
        let first = vec![b'a'; 40_000];
        let second = vec![b'b'; 40_000];
        let mut bytes = frame(1, 1, &first);
        bytes.extend(frame(1, 2, &second));
        assert!(bytes.len() > 65535);

        let mut messages = Messages::new();
        messages.feed(&bytes);

        let message = messages.next_message().unwrap().unwrap();
        assert_eq!((message.id, message.data), (1, first));
        let message = messages.next_message().unwrap().unwrap();
        assert_eq!((message.id, message.data), (2, second));
        assert!(messages.next_message().unwrap().is_none());
        assert!(!messages.is_incomplete());
    }

    #[test]
    fn keeps_partial_header_after_complete_message() {
        let next = frame(1, 2, b"next");
        let mut bytes = frame(1, 1, b"first");
        bytes.extend_from_slice(&next[..2]);

        let mut messages = Messages::new();
        messages.feed(&bytes);

        let message = messages.next_message().unwrap().unwrap();
        assert_eq!(message.data, b"first");
        assert!(messages.next_message().unwrap().is_none());
        assert!(messages.is_incomplete());

        messages.feed(&next[2..]);
        let message = messages.next_message().unwrap().unwrap();
        assert_eq!(message.data, b"next");
        assert!(!messages.is_incomplete());
    }

    #[test]
    fn rejects_declared_size_smaller_than_header() {
        let mut messages = Messages::new();
        messages.feed(&[0, 1, 0, 2, 0, 5]);

        assert!(messages.next_message().is_err());
    }

    #[test]
    fn accepts_header_only_message() {
        let mut messages = Messages::new();
        messages.feed(&[0, 1, 0, 2, 0, 6]);

        let message = messages.next_message().unwrap().unwrap();
        assert_eq!(message.size, 6);
        assert!(message.data.is_empty());
        assert!(!messages.is_incomplete());
    }
}
