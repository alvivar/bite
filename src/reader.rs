use crate::conn::Connection;
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
                            // Loop because sometimes "received" could have more
                            // than one message.
                            loop {
                                self.buffer.append(&mut received);

                                let size = // The first 2 bytes are the size.
                                    (self.buffer[0] as u32) << 8 | (self.buffer[1] as u32) << 0; // BigEndian
                                let buffer_len = self.buffer.len() as u32;

                                println!("Sizes {} / {}", size, buffer_len);

                                // The message is complete!
                                if size == buffer_len {
                                    self.buffer.drain(0..2);

                                    parser_tx
                                        .send(parser::Cmd::Parse(
                                            id,
                                            self.buffer.to_owned(),
                                            conn.addr,
                                        ))
                                        .unwrap();

                                    self.buffer.clear();
                                    break;
                                }
                                // The message contains more than one message,
                                // let's send and loop.
                                else if buffer_len > size {
                                    let split = self.buffer.split_off(size as usize);

                                    self.buffer.drain(0..2);
                                    parser_tx
                                        .send(parser::Cmd::Parse(
                                            id,
                                            self.buffer.to_owned(),
                                            conn.addr,
                                        ))
                                        .unwrap();

                                    self.buffer = split;
                                    let utf8 = String::from_utf8_lossy(&self.buffer);
                                    println!("\nAfter split: {}", utf8);
                                    println!("After split: {:?}", &self.buffer);
                                } else {
                                    let utf8 = String::from_utf8_lossy(&self.buffer);
                                    println!("\nUnusual: {}", utf8);
                                    println!("Unusual: {:?}", &self.buffer);

                                    panic!("Unusual");
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
                        subs_tx.send(DelAll(rcon.keys, id)).unwrap();
                    }
                }
            }
        }
    }
}
