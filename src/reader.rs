use crate::connection::{Connection, Received};
use crate::parser::Action::Parse;
use crate::subs::Action::DelAll;
use crate::{parser, subs};

use crossbeam_channel::{unbounded, Receiver, Sender};
use polling::{Event, Poller};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub enum Action {
    Read(usize),
}

pub struct Reader {
    poller: Arc<Poller>,
    readers: Arc<Mutex<HashMap<usize, Connection>>>,
    writers: Arc<Mutex<HashMap<usize, Connection>>>,
    pub tx: Sender<Action>,
    rx: Receiver<Action>,
}

impl Reader {
    pub fn new(
        poller: Arc<Poller>,
        readers: Arc<Mutex<HashMap<usize, Connection>>>,
        writers: Arc<Mutex<HashMap<usize, Connection>>>,
    ) -> Reader {
        let (tx, rx) = unbounded::<Action>();

        Reader {
            poller,
            readers,
            writers,
            tx,
            rx,
        }
    }

    pub fn handle(&mut self, parser_tx: Sender<parser::Action>, subs_tx: Sender<subs::Action>) {
        loop {
            match self.rx.recv().unwrap() {
                Action::Read(id) => {
                    let mut closed = false;

                    if let Some(connection) = self.readers.lock().unwrap().get_mut(&id) {
                        loop {
                            let mut pending = false;

                            let message = match connection.try_read_message() {
                                Received::None => break,

                                Received::Complete(received) => received,

                                Received::Incomplete(received) => {
                                    pending = true;
                                    received
                                }

                                Received::Error(err) => {
                                    println!("\nConnection #{} closed, read failed: {}", id, err);
                                    break;
                                }
                            };

                            parser_tx.send(Parse(id, message, connection.addr)).unwrap();

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
