use crossbeam_channel::{unbounded, Receiver, Sender};
use mio::{net::TcpStream, Token};

use std::net::SocketAddr;

pub struct Connection {
    pub token: Token,
    pub socket: TcpStream,
    pub address: SocketAddr,
    pub open: bool,
    pub rx: Receiver<Vec<u8>>,
    pub tx: Sender<Vec<u8>>,
}

impl Connection {
    pub fn new(token: Token, socket: TcpStream, address: SocketAddr) -> Connection {
        let (tx, rx) = unbounded::<Vec<u8>>();

        Connection {
            token,
            socket,
            address,
            open: true,
            rx,
            tx,
        }
    }
}
