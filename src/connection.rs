use crate::parser;

use std::cmp::Ordering;
use std::io::ErrorKind::{self, BrokenPipe, Interrupted, WouldBlock, WriteZero};
use std::io::{self, Error, Read, Write};
use std::net::{SocketAddr, TcpStream};

pub enum Message {
    None,
    Some(Vec<u8>),
    Pending(Vec<u8>),
    Error(io::Error),
}

pub struct Connection {
    pub id: usize,
    pub socket: TcpStream,
    pub addr: SocketAddr,
    pub to_send: Vec<Vec<u8>>,
    pub closed: bool,
    buffer: Vec<u8>,
}

impl Connection {
    pub fn new(id: usize, socket: TcpStream, addr: SocketAddr) -> Connection {
        let to_send = Vec::<Vec<u8>>::new();
        let buffer = Vec::<u8>::new();

        Connection {
            id,
            socket,
            addr,
            to_send,
            buffer,
            closed: false,
        }
    }

    /// Reads the socket acting like a buffer to return complete messages
    /// according to the protocol. You need to call this function in a loop and
    /// retry when Response::Pending is returned.
    pub fn try_read_message(&mut self) -> Message {
        match self.try_read() {
            Ok(mut received) => {
                // Loop because "received" could have more than one message in
                // the same read.

                self.buffer.append(&mut received);

                // The first 2 bytes represent the message size.
                let size = parser::get_size(&self.buffer[..]);
                let buffer_len = self.buffer.len() as u32;

                // Bigger than what can be represented in 2 bytes.
                if buffer_len > 65535 {
                    self.closed = true;
                    self.buffer.clear();

                    return Message::Error(Error::new(
                        ErrorKind::Unsupported,
                        "Message received is bigger than 65535 bytes and the protocol uses only 2 bytes to represent the size.",
                    ));
                }

                match size.cmp(&buffer_len) {
                    Ordering::Equal => {
                        // The message is complete, just send it and break.

                        let result = self.buffer.to_owned();
                        self.buffer.clear();

                        Message::Some(result)
                    }

                    Ordering::Less => {
                        // The message received contains more than one message.
                        // Let's split, send the first part and deal with the
                        // rest on the next iteration.

                        let split = self.buffer.split_off(size as usize);
                        let result = self.buffer.to_owned();
                        self.buffer = split;

                        Message::Pending(result)
                    }

                    Ordering::Greater => {
                        // The loop should only happen when we need to unpack
                        // more than one message received in the same read, else
                        // break to deal with the buffer or new messages.

                        Message::None
                    }
                }
            }

            Err(err) => Message::Error(err),
        }
    }

    pub fn try_write_message(&mut self, data: Vec<u8>) -> io::Result<usize> {
        let data = parser::insert_size(data);
        self.try_write(data)
    }

    fn try_read(&mut self) -> io::Result<Vec<u8>> {
        match read(&mut self.socket) {
            Ok(data) => Ok(data),

            Err(err) => {
                self.closed = true;

                Err(err)
            }
        }
    }

    fn try_write(&mut self, data: Vec<u8>) -> io::Result<usize> {
        match write(&mut self.socket, data) {
            Ok(count) => Ok(count),

            Err(err) => {
                self.closed = true;

                Err(err)
            }
        }
    }
}

// Both functions below were taken and modified from the Tokio/mio client server
// example.

fn read(socket: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut received = vec![0; 4096]; // @todo What could be the correct size for this?
    let mut bytes_read = 0;

    loop {
        match socket.read(&mut received[bytes_read..]) {
            Ok(0) => {
                // Reading 0 bytes means the other side has closed the
                // connection or is done writing, then so are we.
                return Err(BrokenPipe.into());
            }

            Ok(n) => {
                bytes_read += n;
                if bytes_read == received.len() {
                    received.resize(received.len() + 1024, 0);
                }
            }

            // Would block "errors" are the OS's way of saying that the
            // connection is not actually ready to perform this I/O operation.
            Err(ref err) if err.kind() == WouldBlock => break,

            // Got interrupted (how rude!), we'll try again.
            Err(ref err) if err.kind() == Interrupted => continue,

            // Other errors we'll consider fatal.
            Err(err) => return Err(err),
        }
    }

    received.truncate(bytes_read);

    Ok(received)
}

fn write(socket: &mut TcpStream, data: Vec<u8>) -> io::Result<usize> {
    match socket.write(&data) {
        // We want to write the entire `DATA` buffer in a single go. If we write
        // less we'll return a short write error (same as `io::Write::write_all`
        // does).
        Ok(n) if n < data.len() => Err(WriteZero.into()),

        Ok(n) => Ok(n),

        // Would block "errors" are the OS's way of saying that the connection
        // is not actually ready to perform this I/O operation.
        Err(ref err) if err.kind() == WouldBlock => Err(WouldBlock.into()),

        // Got interrupted (how rude!), we'll try again.
        Err(ref err) if err.kind() == Interrupted => write(socket, data),

        // Other errors we'll consider fatal.
        Err(err) => Err(err),
    }
}
