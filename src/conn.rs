use std::net::{SocketAddr, TcpStream};

pub struct Connection {
    pub id: usize,
    pub socket: TcpStream,
    pub addr: SocketAddr,
    pub data: Vec<u8>, // @todo This should be probably a list of Vecs because of concurrency.
    pub closed: bool,
}

impl Connection {
    pub fn new(id: usize, socket: TcpStream, addr: SocketAddr) -> Connection {
        let cache = Vec::<u8>::new();

        Connection {
            id,
            socket,
            addr,
            data: cache,
            closed: false,
        }
    }
}
