use crate::conn::{self, Connection};
use crate::parser::Cmd::Parse;
use crate::subs::Cmd::DelAll;
use crate::{parser, subs};

use crossbeam_channel::{unbounded, Receiver, Sender};
use polling::{Event, Poller};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub enum Cmd {
    Read(usize),
}

pub struct Reader {
    poller: Arc<Poller>,
    readers: Arc<Mutex<HashMap<usize, Connection>>>,
    writers: Arc<Mutex<HashMap<usize, Connection>>>,
    pub tx: Sender<Cmd>,
    rx: Receiver<Cmd>,
}

impl Reader {
    pub fn new(
        poller: Arc<Poller>,
        readers: Arc<Mutex<HashMap<usize, Connection>>>,
        writers: Arc<Mutex<HashMap<usize, Connection>>>,
    ) -> Reader {
        let (tx, rx) = unbounded::<Cmd>();

        Reader {
            poller,
            readers,
            writers,
            tx,
            rx,
        }
    }

    pub fn handle(&mut self, parser_tx: Sender<parser::Cmd>, subs_tx: Sender<subs::Cmd>) {
        loop {
            match self.rx.recv().unwrap() {
                Cmd::Read(id) => {
                    let mut closed = false;

                    if let Some(connection) = self.readers.lock().unwrap().get_mut(&id) {
                        loop {
                            let mut pending = false;

                            let received = match connection.try_read_message() {
                                conn::Response::None => break,

                                conn::Response::Some(received) => received,

                                conn::Response::Pending(received) => {
                                    pending = true;
                                    received
                                }

                                conn::Response::Error(err) => {
                                    println!("\nConnection #{} broken, read failed: {}", id, err);
                                    break;
                                }
                            };

                            parser_tx
                                .send(Parse(id, received, connection.addr))
                                .unwrap();

                            if !pending {
                                break;
                            }
                        }

                        if connection.closed {
                            closed = true;
                        } else {
                            self.poller
                                .modify(&connection.socket, Event::readable(id))
                                .unwrap();
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
}
