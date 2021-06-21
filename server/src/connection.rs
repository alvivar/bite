use std::net::SocketAddr;

use mio::{net::TcpStream, Token};

pub struct Connection {
    pub token: Token,
    pub socket: TcpStream,
    pub address: SocketAddr,
    pub open: bool,
    pub to_send: Vec<u8>,
}

impl Connection {
    pub fn new(token: Token, socket: TcpStream, address: SocketAddr) -> Connection {
        Connection {
            token,
            socket,
            address,
            open: true,
            to_send: Vec::new(),
        }
    }
}
