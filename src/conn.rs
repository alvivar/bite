use std::net::{SocketAddr, TcpStream};

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
}
