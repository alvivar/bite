use std::{
    io::{
        self,
        ErrorKind::{BrokenPipe, Interrupted, WouldBlock},
        Read, Write,
    },
    net::{SocketAddr, TcpStream},
};

pub struct Connection {
    pub id: usize,
    pub socket: TcpStream,
    pub addr: SocketAddr,
    pub keys: Vec<String>, // Only Readers know the keys in the current algorithm.
    pub received: Vec<Vec<u8>>,
    pub to_write: Vec<Vec<u8>>,
    pub closed: bool,
}

impl Connection {
    pub fn new(id: usize, socket: TcpStream, addr: SocketAddr) -> Connection {
        let keys = Vec::<String>::new();
        let received = Vec::<Vec<u8>>::new();
        let to_write = Vec::<Vec<u8>>::new();

        Connection {
            id,
            socket,
            addr,
            keys,
            received,
            to_write,
            closed: false,
        }
    }

    pub fn try_read(&mut self) {
        let data = match read(&mut self.socket) {
            Ok(data) => data,
            Err(err) => {
                println!("Connection #{} broken, read failed: {}", self.id, err);
                self.closed = true;
                return;
            }
        };

        self.received.push(data);
    }

    pub fn try_write(&mut self) {
        let data = self.to_write.remove(0);

        if let Err(err) = self.socket.write(&data) {
            println!("Connection #{} broken, write failed: {}", self.id, err);
            self.closed = true;
        }
    }
}

fn read(socket: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut received = vec![0; 1024 * 4];
    let mut bytes_read = 0;

    loop {
        match socket.read(&mut received[bytes_read..]) {
            Ok(0) => {
                // Reading 0 bytes means the other side has closed the
                // connection or is done writing, then so are we.
                return Err(io::Error::new(BrokenPipe, "0 bytes read"));
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

    // let received_data = &received_data[..bytes_read];
    // @doubt Using this slice ^ thing and returning with into() versus using
    // the resize?

    received.resize(bytes_read, 0);

    Ok(received)
}
