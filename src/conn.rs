use std::cmp::Ordering;
use std::io::ErrorKind::{BrokenPipe, Interrupted, WouldBlock, WriteZero};
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};

pub enum Response {
    None,
    Some(Vec<u8>),
    Pending(Vec<u8>),
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

    /// Returns the complete message according to the protocol, even if needs
    /// multiple reads from the socket.
    pub fn try_read_message(&mut self) -> Response {
        if let Some(mut received) = self.try_read() {
            // Loop because sometimes "received" could have more than one
            // message in the same read.

            self.buffer.append(&mut received);

            // The first 2 bytes represent the message size.
            let size = (self.buffer[0] as u32) << 8 | (self.buffer[1] as u32); // BigEndian
            let buffer_len = self.buffer.len() as u32;

            match size.cmp(&buffer_len) {
                Ordering::Equal => {
                    // The message is complete, just send it and break.

                    self.buffer.drain(0..2);
                    let result = self.buffer.to_owned();
                    self.buffer.clear();

                    return Response::Some(result);
                }

                Ordering::Less => {
                    // The message received contains more than one message.
                    // Let's split, send the first part and deal with the rest
                    // on the next iteration.

                    let split = self.buffer.split_off(size as usize);
                    self.buffer.drain(0..2);
                    let result = self.buffer.to_owned();
                    self.buffer = split;

                    return Response::Pending(result);
                }

                Ordering::Greater => {
                    // The loop should only happen when we need to unpack
                    // more than one message received in the same read, else
                    // break to deal with the buffer or new messages.
                    return Response::None;
                }
            }
        }

        Response::None
    }

    pub fn try_read(&mut self) -> Option<Vec<u8>> {
        let data = match read(&mut self.socket) {
            Ok(data) => data,
            Err(err) => {
                println!("Connection #{} broken, read failed: {}", self.id, err);
                self.closed = true;
                return None;
            }
        };

        Some(data)
    }

    pub fn try_write(&mut self, data: Vec<u8>) {
        // @todo Should we propagate the ammount of bytes written instead of
        // only catching the error?

        if let Err(err) = write(&mut self.socket, data) {
            println!("Connection #{} broken, write failed: {}", self.id, err);
            self.closed = true;
        }
    }
}

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

    // @todo Do we really need to truncate? Probably because if we grew

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
