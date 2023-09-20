use std::{
    io::{
        self,
        ErrorKind::{BrokenPipe, Interrupted, WouldBlock},
        Read, Write,
    },
    net::{SocketAddr, TcpStream},
    time::Instant,
};

const BUFFER_SIZE: usize = 4096;

pub struct Connection {
    pub id: usize,
    pub socket: TcpStream,
    pub addr: SocketAddr,
    pub send_queue: Vec<Vec<u8>>,
    pub pending_read: bool,
    pub last_read: Instant,
    pub last_write: Instant,
    pub closed: bool,
}

impl Connection {
    pub fn new(id: usize, socket: TcpStream, addr: SocketAddr) -> Connection {
        let send_queue = Vec::<Vec<u8>>::new();

        Connection {
            id,
            socket,
            addr,
            send_queue,
            pending_read: false,
            last_read: Instant::now(),
            last_write: Instant::now(),
            closed: false,
        }
    }

    pub fn try_read(&mut self) -> io::Result<Vec<u8>> {
        match read(&mut self.socket) {
            Ok(data) => Ok(data),

            Err(err) => {
                self.closed = true;

                Err(err)
            }
        }
    }

    pub fn try_write(&mut self, data: Vec<u8>) -> io::Result<usize> {
        match write(&mut self.socket, data) {
            Ok(count) => Ok(count),

            Err(err) => {
                self.closed = true;

                Err(err)
            }
        }
    }
}

fn read(socket: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut buffer = Vec::with_capacity(BUFFER_SIZE);

    loop {
        let mut chunk = vec![0; BUFFER_SIZE];

        match socket.read(&mut chunk) {
            Ok(0) => {
                // Reading 0 bytes means the other side has closed the
                // connection or is done writing, then so are we.
                return Err(BrokenPipe.into());
            }

            Ok(n) => {
                buffer.extend_from_slice(&chunk[..n]);

                if n < BUFFER_SIZE {
                    break;
                }
            }

            // Would block "errors" are the OS's way of saying that the
            // connection is not actually ready to perform this I/O operation.
            Err(ref err) if err.kind() == WouldBlock => break,

            // Got interrupted, we'll try again.
            Err(ref err) if err.kind() == Interrupted => continue,

            // Other errors we'll consider fatal.
            Err(err) => return Err(err),
        }
    }

    Ok(buffer)
}

fn write(socket: &mut TcpStream, data: Vec<u8>) -> io::Result<usize> {
    let mut total_written = 0;

    while total_written < data.len() {
        match socket.write(&data[total_written..]) {
            Ok(0) => {
                // Writing 0 bytes means the other side has closed the
                // connection or is done writing, then so are we.
                return Err(BrokenPipe.into());
            }

            Ok(n) => total_written += n,

            // Would block "errors" are the OS's way of saying that the
            // connection is not actually ready to perform this I/O operation.
            Err(ref err) if err.kind() == WouldBlock => return Err(WouldBlock.into()),

            // Got interrupted, we'll try again.
            Err(ref err) if err.kind() == Interrupted => continue,

            // Other errors we'll consider fatal.
            Err(err) => return Err(err),
        }
    }

    Ok(total_written)
}
