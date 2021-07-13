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
    Write(usize, String),
    WriteAll(Vec<usize>, String),
}

pub struct Writer {
    writers: Arc<Mutex<HashMap<usize, Connection>>>,
    poller: Arc<Poller>,
    pub tx: Sender<Cmd>,
    rx: Receiver<Cmd>,
}

impl Writer {
    pub fn new(writers: Arc<Mutex<HashMap<usize, Connection>>>, poller: Arc<Poller>) -> Writer {
        let (tx, rx) = channel::<Cmd>();

        Writer {
            writers,
            poller,
            tx,
            rx,
        }
    }

    pub fn handle(self) {
        loop {
            let cmd = self.rx.recv().unwrap();
            match cmd {
                Cmd::Write(id, msg) => {
                    if let Some(conn) = self.writers.lock().unwrap().get_mut(&id) {
                        conn.data = msg.into();

                        self.poller
                            .modify(&conn.socket, Event::writable(conn.id))
                            .unwrap();
                    }
                }

                Cmd::WriteAll(ids, msg) => {
                    let mut writers = self.writers.lock().unwrap();

                    for id in ids {
                        if let Some(conn) = writers.get_mut(&id) {
                            conn.data = msg.to_owned().into();

                            self.poller
                                .modify(&conn.socket, Event::writable(conn.id))
                                .unwrap();
                        }
                    }
                }
            }
        }
    }
}
