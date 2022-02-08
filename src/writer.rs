use std::{
    collections::HashMap,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
};

use polling::Poller;

use crate::{conn::Connection, subs};

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
    readers: Arc<Mutex<HashMap<usize, Connection>>>,
    poller: Arc<Poller>,
    pub tx: Sender<Cmd>,
    rx: Receiver<Cmd>,
}

impl Writer {
    pub fn new(
        poller: Arc<Poller>,
        writers: Arc<Mutex<HashMap<usize, Connection>>>,
        readers: Arc<Mutex<HashMap<usize, Connection>>>,
    ) -> Writer {
        let (tx, rx) = channel::<Cmd>();

        Writer {
            writers,
            readers,
            poller,
            tx,
            rx,
        }
    }

    pub fn handle(&self, subs_tx: Sender<subs::Cmd>) {
        loop {
            match self.rx.recv().unwrap() {
                Cmd::Write(id, msg) => {
                    self.tx.send(Cmd::WriteAll(vec![Msg { id, msg }])).unwrap();
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
                        let rconn = self.readers.lock().unwrap().remove(&id).unwrap();
                        let wconn = self.writers.lock().unwrap().remove(&id).unwrap();
                        self.poller.delete(&rconn.socket).unwrap();
                        self.poller.delete(&wconn.socket).unwrap();
                        subs_tx.send(subs::Cmd::DelAll(rconn.keys, id)).unwrap();
                    }
                }
            }
        }
    }
}
