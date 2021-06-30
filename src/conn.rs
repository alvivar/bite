use std::net::{SocketAddr, TcpStream};

pub struct Connection {
    pub id: usize,
    pub socket: TcpStream,
    pub addr: SocketAddr,
}

impl Connection {
    pub fn new(id: usize, socket: TcpStream, addr: SocketAddr) -> Connection {
        Connection { id, socket, addr }
    }
}
