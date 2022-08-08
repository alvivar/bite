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
                Cmd::Queue(id, message) => {
                    if let Some(connection) = self.writers.lock().unwrap().get_mut(&id) {
                        connection.to_send.push(message);
                        self.poll_write(connection);
                    }
                }

                Cmd::QueueAll(messages) => {
                    let mut writers = self.writers.lock().unwrap();
                    for message in messages {
                        if let Some(connection) = writers.get_mut(&message.id) {
                            connection.to_send.push(message.data);
                            self.poll_write(connection);
                        }
                    }
                }

                Cmd::Write(id) => {
                    let mut closed = false;
                    if let Some(connection) = self.writers.lock().unwrap().get_mut(&id) {
                        if !connection.to_send.is_empty() {
                            let data = connection.to_send.remove(0);
                            connection.try_write_message(data);
                        }

                        if connection.closed {
                            closed = true;
                        } else if !connection.to_send.is_empty() {
                            self.poll_write(connection);
                        } else {
                            self.poll_clean(connection);
                        }
                    }

                    if closed {
                        let readers = self.readers.lock().unwrap().remove(&id).unwrap();
                        let writers = self.writers.lock().unwrap().remove(&id).unwrap();
                        self.poller.delete(&readers.socket).unwrap();
                        self.poller.delete(&writers.socket).unwrap();
                        subs_tx.send(DelAll(id)).unwrap();
                    }
                }
            }
        }
    }

    fn poll_write(&self, connection: &mut Connection) {
        self.poller
            .modify(&connection.socket, Event::writable(connection.id))
            .unwrap();
    }

    fn poll_clean(&self, connection: &mut Connection) {
        self.poller
            .modify(&connection.socket, Event::none(connection.id))
            .unwrap();
    }
}
