use crate::conn::Connection;
use crate::subs::{self, Cmd::DelAll};

use crossbeam_channel::{unbounded, Receiver, Sender};
use polling::{Event, Poller};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct Message {
    pub id: usize,
    pub data: Vec<u8>,
}

pub enum Cmd {
    Queue(usize, Vec<u8>),
    QueueAll(Vec<Message>),
    Write(usize),
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
                Cmd::Queue(id, mut message) => {
                    if let Some(conn) = self.writers.lock().unwrap().get_mut(&id) {
                        message.push(b'\n');
                        conn.to_send.push(message);
                        self.poll_write(conn);
                    }
                }

                Cmd::QueueAll(messages) => {
                    let mut writers = self.writers.lock().unwrap();
                    for message in messages {
                        if let Some(conn) = writers.get_mut(&message.id) {
                            let mut message = message.data;
                            message.push(b'\n');
                            conn.to_send.push(message);
                            self.poll_write(conn);
                        }
                    }
                }

                Cmd::Write(id) => {
                    let mut closed = false;
                    if let Some(conn) = self.writers.lock().unwrap().get_mut(&id) {
                        if !conn.to_send.is_empty() {
                            let data = conn.to_send.remove(0);
                            conn.try_write(data);
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
                        subs_tx.send(DelAll(id)).unwrap();
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
