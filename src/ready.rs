use std::{
    collections::HashMap,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
};

use polling::{Event, Poller};

use crate::conn::Connection;

pub enum Cmd {
    Read(Connection),
    Write(Connection),
}

pub struct Ready {
    poller: Arc<Poller>,
    reader_map: Arc<Mutex<HashMap<usize, Connection>>>,
    writer_map: Arc<Mutex<HashMap<usize, Connection>>>,
    pub tx: Sender<Cmd>,
    rx: Receiver<Cmd>,
}

impl Ready {
    pub fn new(
        poller: Arc<Poller>,
        reader_map: Arc<Mutex<HashMap<usize, Connection>>>,
        writer_map: Arc<Mutex<HashMap<usize, Connection>>>,
    ) -> Ready {
        let (tx, rx) = channel::<Cmd>();

        Ready {
            poller,
            reader_map,
            writer_map,
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

                    self.reader_map.lock().unwrap().insert(conn.id, conn);
                }

                Cmd::Write(conn) => {
                    self.writer_map.lock().unwrap().insert(conn.id, conn);
                }
            }
        }
    }
}
