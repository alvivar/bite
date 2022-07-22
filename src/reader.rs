use crate::conn::Connection;
use crate::parser::Cmd::Parse;
use crate::subs::Cmd::DelAll;
use crate::{parser, subs};

use crossbeam_channel::{unbounded, Receiver, Sender};
use polling::{Event, Poller};

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub enum Cmd {
    Read(usize),
}

pub struct Reader {
    poller: Arc<Poller>,
    readers: Arc<Mutex<HashMap<usize, Connection>>>,
    writers: Arc<Mutex<HashMap<usize, Connection>>>,
    buffer: Vec<u8>,
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
        let buffer = Vec::new();

        Reader {
            poller,
            readers,
            writers,
            buffer,
            tx,
            rx,
        }
    }

    pub fn handle(&mut self, parser_tx: Sender<parser::Cmd>, subs_tx: Sender<subs::Cmd>) {
        loop {
            match self.rx.recv().unwrap() {
                Cmd::Read(id) => {
                    let mut closed = false;
                    if let Some(conn) = self.readers.lock().unwrap().get_mut(&id) {
                        if let Some(mut received) = conn.try_read() {
                            loop {
                                // Loop because sometimes "received" could have
                                // more than one message in the same read.

                                self.buffer.append(&mut received);

                                // The first 2 bytes represent the message size.
                                let size = (self.buffer[0] as u32) << 8 | (self.buffer[1] as u32); // BigEndian
                                let buffer_len = self.buffer.len() as u32;

                                match size.cmp(&buffer_len) {
                                    Ordering::Equal => {
                                        // The message is complete, just send it
                                        // and break.

                                        self.buffer.drain(0..2);

                                        parser_tx
                                            .send(Parse(id, self.buffer.to_owned(), conn.addr))
                                            .unwrap();

                                        self.buffer.clear();

                                        break;
                                    }

                                    Ordering::Less => {
                                        // The message received contains more
                                        // than one message, let's split, send
                                        // the first part and deal with the rest
                                        // on the next loop.

                                        let split = self.buffer.split_off(size as usize);

                                        self.buffer.drain(0..2);
                                        parser_tx
                                            .send(Parse(id, self.buffer.to_owned(), conn.addr))
                                            .unwrap();

                                        self.buffer = split;
                                    }

                                    _ => {
                                        // The loop should only happen when we
                                        // need to unpack more than one message
                                        // received in the same read, else break
                                        // to deal with the buffer or new
                                        // messages.
                                        break;
                                    }
                                }
                            }
                        }

                        if conn.closed {
                            closed = true;
                        } else {
                            self.poller
                                .modify(&conn.socket, Event::readable(id))
                                .unwrap();
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
}
