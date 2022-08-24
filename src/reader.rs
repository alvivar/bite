use crate::connection::Connection;
use crate::message::{Message, Messages, Received};
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
    messages: Messages,
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
        let messages = Messages::new();

        Reader {
            poller,
            readers,
            writers,
            messages,
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
                            // Loop because "received" could have more than one
                            // message in the same read.

                            let mut pending = false;

                            let data = match connection.try_read() {
                                Ok(received) => received,

                                Err(err) => {
                                    println!("\nConnection #{} closed, read failed: {}", id, err);
                                    break;
                                }
                            };

                            let received = match self.messages.feed(data) {
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

                            let message = Message::from_protocol(received);
                            parser_tx.send(Parse(message, connection.addr)).unwrap();

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
