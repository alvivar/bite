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
            writers,
            readers,
            tx,
            rx,
        }
    }

    pub fn handle(&self, parser_tx: Sender<parser::Cmd>, subs_tx: Sender<subs::Cmd>) {
        loop {
            match self.rx.recv().unwrap() {
                Cmd::Read(id) => {
                    let mut closed = false;
                    if let Some(conn) = self.readers.lock().unwrap().get_mut(&id) {
                        if let Some(received) = conn.try_read() {
                            // @todo Here we should start collecting messages
                            // based on the size of the message, and send the
                            // complete data to the parser thread to be handled.

                            // The first 2 bytes are the size.

                            let size = ((received[0] as u32) << 8) + ((received[1] as u32) << 0);
                            println!("Size from received: {}", size);

                            parser_tx
                                .send(parser::Cmd::Parse(id, received, conn.addr))
                                .unwrap();
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
