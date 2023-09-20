use std::{
    collections::HashMap,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
    time::Instant,
};

use crate::{
    cleaner,
    connection::Connection,
    message::{Message, Messages, Received},
    parser::{self, Action::Parse},
};

use polling::{Event, Poller};

pub enum Action {
    Read(usize),
}

pub struct Reader {
    poller: Arc<Poller>,
    readers: Arc<Mutex<HashMap<usize, Connection>>>,
    messages: HashMap<usize, Messages>,
    pub tx: Sender<Action>,
    rx: Receiver<Action>,
}

impl Reader {
    pub fn new(poller: Arc<Poller>, readers: Arc<Mutex<HashMap<usize, Connection>>>) -> Reader {
        let messages = HashMap::<usize, Messages>::new();
        let (tx, rx) = channel::<Action>();

        Reader {
            poller,
            readers,
            messages,
            tx,
            rx,
        }
    }

    pub fn handle(
        &mut self,
        parser_tx: Sender<parser::Action>,
        cleaner_tx: Sender<cleaner::Action>,
    ) {
        loop {
            match self.rx.recv().unwrap() {
                Action::Read(id) => {
                    let mut closed = false;

                    if let Some(connection) = self.readers.lock().unwrap().get_mut(&id) {
                        loop {
                            // Loop because "received" could have more than one
                            // message in the same read.

                            let data = match connection.try_read() {
                                Ok(received) => received,

                                Err(err) => {
                                    // connection.closed = true;
                                    // ^ This is already hapenning inside try_read() on errors.

                                    info!("Connection #{id} closed, read failed: {err}");

                                    break;
                                }
                            };

                            let mut pending = false;
                            let messages = self.messages.entry(id).or_insert_with(Messages::new);

                            let received = match messages.feed(data) {
                                Received::None => break,

                                Received::Complete(received) => {
                                    connection.pending_read = false;
                                    connection.last_read = Instant::now();
                                    received
                                }

                                Received::Pending(received) => {
                                    pending = true;
                                    connection.pending_read = true;
                                    connection.last_read = Instant::now();

                                    received
                                }

                                Received::Error(err) => {
                                    connection.closed = true;

                                    info!("Connection #{id} closed, feed failed: {err}");

                                    break;
                                }
                            };

                            let message = match Message::from_protocol(received) {
                                Ok(message) if message.from != id as u32 => {
                                    connection.closed = true;

                                    let err = format!("message client id #{} is wrong", message.id);
                                    info!("Connection #{id} closed, bad message: {err}");

                                    break;
                                }

                                Ok(message) => message,

                                Err(err) => {
                                    connection.closed = true;

                                    info!("Connection #{id} closed, bad message: {err}");

                                    break;
                                }
                            };

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
                        self.messages.remove(&id);
                        cleaner_tx.send(cleaner::Action::Drop(id)).unwrap();
                    }
                }
            }
        }
    }
}
