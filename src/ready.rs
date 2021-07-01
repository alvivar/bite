use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crossbeam_channel::{unbounded, Receiver, Sender};
use polling::{Event, Poller};

use crate::conn::Connection;

pub enum Cmd {
    Read(Connection),
    Write(Connection),
}

pub struct Ready {
    poller: Arc<Poller>,
    read_map: Arc<Mutex<HashMap<usize, Connection>>>,
    write_map: Arc<Mutex<HashMap<usize, Connection>>>,
    pub tx: Sender<Cmd>,
    rx: Receiver<Cmd>,
}

impl Ready {
    pub fn new(
        poller: Arc<Poller>,
        read_map: Arc<Mutex<HashMap<usize, Connection>>>,
        write_map: Arc<Mutex<HashMap<usize, Connection>>>,
    ) -> Ready {
        let (tx, rx) = unbounded::<Cmd>();

        Ready {
            poller,
            read_map,
            write_map,
            tx,
            rx,
        }
    }

    pub fn handle(self) {
        loop {
            match self.rx.recv().unwrap() {
                Cmd::Read(conn) => {
                    self.poller
                        .modify(&conn.socket, Event::readable(conn.id))
                        .unwrap();

                    self.read_map.lock().unwrap().insert(conn.id, conn);
                }

                Cmd::Write(conn) => {
                    self.write_map.lock().unwrap().insert(conn.id, conn);
                }
            }
        }
    }
}
