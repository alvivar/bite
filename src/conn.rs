use std::io::ErrorKind::{BrokenPipe, Interrupted, WouldBlock, WriteZero};
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};

pub struct Connection {
    pub id: usize,
    pub socket: TcpStream,
    pub addr: SocketAddr,
    pub keys: Vec<String>, // Only Readers know the keys in the current algorithm.
    pub to_send: Vec<String>,
    pub closed: bool,
}

impl Connection {
    pub fn new(id: usize, socket: TcpStream, addr: SocketAddr) -> Connection {
        let keys = Vec::<String>::new();
        let to_send = Vec::<String>::new();

        Connection {
            id,
            socket,
            addr,
            keys,
            to_send,
            closed: false,
        }
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
        if let Err(err) = write(&mut self.socket, data) {
            println!("Connection #{} broken, write failed: {}", self.id, err);
            self.closed = true;
        }
    }
}

fn read(socket: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut received = vec![0; 1024]; // @todo What could be the correct size for this?
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

            Err(ref err) if err.kind() == Interrupted => continue,

            // Other errors we'll consider fatal.
            Err(err) => return Err(err),
        }
    }

    received.resize(bytes_read, 0);

    Ok(received)
}

fn write(socket: &mut TcpStream, data: Vec<u8>) -> io::Result<usize> {
    match socket.write(&data) {
        // We want to write the entire `DATA` buffer in a single go. If we
        // write less we'll return a short write error (same as
        // `io::Write::write_all` does).
        Ok(n) if n < data.len() => Err(WriteZero.into()),

        Ok(n) => {
            // After we've written something we'll reregister the connection to
            // only respond to readable events.
            Ok(n)
        }

        // Would block "errors" are the OS's way of saying that the connection
        // is not actually ready to perform this I/O operation.
        Err(ref err) if err.kind() == WouldBlock => Err(WouldBlock.into()),

        // Got interrupted (how rude!), we'll try again.
        Err(ref err) if err.kind() == Interrupted => write(socket, data),

        // Other errors we'll consider fatal.
        Err(err) => Err(err),
    }
}
