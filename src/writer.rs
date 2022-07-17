use crate::conn::Connection;
use crate::subs::{self, Cmd::DelAll};

use crossbeam_channel::{unbounded, Receiver, Sender};
use polling::{Event, Poller};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct Msg {
    pub id: usize,
    pub msg: String,
}

pub enum Cmd {
    Push(usize, String),
    PushAll(Vec<Msg>),
    Send(usize), // @todo Maybe Vec<u8> would be better than String.
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
        let (tx, rx) = unbounded::<Cmd>();

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
                Cmd::Push(id, msg) => {
                    if let Some(conn) = self.writers.lock().unwrap().get_mut(&id) {
                        let mut msg = msg.trim_end().to_owned();
                        msg.push('\n');
                        conn.to_send.push(msg);
                        self.poll_write(conn);
                    }
                }

                Cmd::PushAll(msgs) => {
                    let mut writers = self.writers.lock().unwrap();
                    for msg in msgs {
                        if let Some(conn) = writers.get_mut(&msg.id) {
                            let mut msg = msg.msg.trim_end().to_owned();
                            msg.push('\n');
                            conn.to_send.push(msg);
                            self.poll_write(conn);
                        }
                    }
                }

                Cmd::Send(id) => {
                    // @todo Wondering if this could be batched?

                    let mut closed = false;
                    if let Some(conn) = self.writers.lock().unwrap().get_mut(&id) {
                        if !conn.to_send.is_empty() {
                            let msg = conn.to_send.remove(0);
                            conn.try_write(msg.into());
                        }

                        if conn.closed {
                            closed = true;
                        } else if !conn.to_send.is_empty() {
                            self.poll_write(conn);
                        } else {
                            self.poll_clean(conn);
                        }
                    }

                    if closed {
                        let rcon = self.readers.lock().unwrap().remove(&id).unwrap();
                        let wcon = self.writers.lock().unwrap().remove(&id).unwrap();
                        self.poller.delete(&rcon.socket).unwrap();
                        self.poller.delete(&wcon.socket).unwrap();
                        subs_tx.send(DelAll(rcon.keys, id)).unwrap();
                    }
                }
            }
        }
    }

    pub fn poll_write(&self, conn: &mut Connection) {
        self.poller
            .modify(&conn.socket, Event::writable(conn.id))
            .unwrap();
    }

    pub fn poll_clean(&self, conn: &mut Connection) {
        self.poller
            .modify(&conn.socket, Event::none(conn.id))
            .unwrap();
    }
}
