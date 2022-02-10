use std::{
    collections::HashMap,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
};

use polling::Poller;

use crate::{
    conn::Connection,
    subs::{self, Cmd::DelAll},
};

pub struct Msg {
    pub id: usize,
    pub msg: String,
}

pub enum Cmd {
    Write(usize, String), // @todo Maybe Vec<u8> would be better than String.
    WriteAll(Vec<Msg>),
}

pub struct Writer {
    poller: Arc<Poller>,
    readers: Arc<Mutex<HashMap<usize, Connection>>>,
    writers: Arc<Mutex<HashMap<usize, Connection>>>,
    pub tx: Sender<Cmd>,
    rx: Receiver<Cmd>,
}

impl Writer {
    pub fn new(
        poller: Arc<Poller>,
        readers: Arc<Mutex<HashMap<usize, Connection>>>,
        writers: Arc<Mutex<HashMap<usize, Connection>>>,
    ) -> Writer {
        let (tx, rx) = channel::<Cmd>();

        Writer {
            poller,
            writers,
            readers,
            tx,
            rx,
        }
    }

    pub fn handle(&self, subs_tx: Sender<subs::Cmd>) {
        loop {
            match self.rx.recv().unwrap() {
                Cmd::Write(id, msg) => {
                    let mut closed = false;

                    if let Some(conn) = self.writers.lock().unwrap().get_mut(&id) {
                        let mut msg = msg.trim_end().to_owned();
                        msg.push('\n');

                        conn.try_write(msg.into());

                        if conn.closed {
                            closed = true;
                        }
                    }

                    if closed {
                        self.writers.lock().unwrap().remove(&id).unwrap();
                        let rconn = self.readers.lock().unwrap().remove(&id).unwrap();
                        self.poller.delete(&rconn.socket).unwrap();
                        subs_tx.send(DelAll(rconn.keys, id)).unwrap();
                    }
                }

                Cmd::WriteAll(msgs) => {
                    let mut closed = Vec::<usize>::new();
                    let mut writers = self.writers.lock().unwrap();

                    for msg in msgs {
                        if let Some(conn) = writers.get_mut(&msg.id) {
                            let mut msg = msg.msg.trim_end().to_owned();
                            msg.push('\n');

                            conn.try_write(msg.into());

                            if conn.closed {
                                closed.push(conn.id);
                            }
                        }
                    }

                    for id in closed {
                        self.writers.lock().unwrap().remove(&id).unwrap();
                        let rconn = self.readers.lock().unwrap().remove(&id).unwrap();
                        self.poller.delete(&rconn.socket).unwrap();
                        subs_tx.send(DelAll(rconn.keys, id)).unwrap();
                    }
                }
            }
        }
    }
}
