use std::{
    collections::HashMap,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
};

use polling::{Event, Poller};

use crate::conn::Connection;

pub struct Msg {
    pub id: usize,
    pub msg: String,
}

pub enum Cmd {
    Write(usize, String), // @todo Maybe Vec<u8> would be better than String.
    WriteAll(Vec<Msg>),
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
            match self.rx.recv().unwrap() {
                Cmd::Write(id, msg) => {
                    if let Some(conn) = self.writers.lock().unwrap().get_mut(&id) {
                        let mut msg = msg.trim_end().to_owned();
                        msg.push('\n');
                        conn.to_write.push(msg.into());

                        self.poller
                            .modify(&conn.socket, Event::writable(conn.id))
                            .unwrap();
                    }
                }

                Cmd::WriteAll(msgs) => {
                    let mut writers = self.writers.lock().unwrap();

                    for msg in msgs {
                        if let Some(conn) = writers.get_mut(&msg.id) {
                            let mut msg = msg.msg.trim_end().to_owned();
                            msg.push('\n');
                            conn.to_write.push(msg.into());

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
