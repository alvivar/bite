use std::{
    collections::HashMap,
    str::from_utf8,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
};

use polling::{Event, Poller};

use crate::{
    conn::Connection,
    data::{
        self,
        Cmd::{Append, Bite, Delete, Get, Inc, Json, Jtrim, Set, SetIfNone},
    },
    msg::{needs_key, parse, Instr},
    subs::{
        self,
        Cmd::{Add, Call, Del, DelAll},
    },
    writer::{self, Cmd::Write},
};

const OK: &str = "OK";
const NOP: &str = "NOP";
const KEY: &str = "KEY?";

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
        let (tx, rx) = channel::<Cmd>();

        Reader {
            poller,
            writers,
            readers,
            tx,
            rx,
        }
    }

    pub fn handle(
        &self,
        data_tx: Sender<data::Cmd>,
        writer_tx: Sender<writer::Cmd>,
        subs_tx: Sender<subs::Cmd>,
    ) {
        loop {
            match self.rx.recv().unwrap() {
                Cmd::Read(id) => {
                    let mut closed = false;
                    if let Some(conn) = self.readers.lock().unwrap().get_mut(&id) {
                        if let Some(received) = conn.try_read() {
                            if let Ok(utf8) = from_utf8(&received) {
                                // We assume multiple instructions separated with newlines.
                                for batched in utf8.trim().split('\n') {
                                    let text = batched.trim();
                                    if text.is_empty() {
                                        continue;
                                    }

                                    let msg = parse(text);
                                    let instr = msg.instr;
                                    let key = msg.key;
                                    let value = msg.value;

                                    match instr {
                                        // Instructions that doesn't make sense without key.
                                        _ if key.is_empty() && needs_key(&instr) => {
                                            writer_tx.send(Write(conn.id, KEY.into())).unwrap();
                                        }

                                        // Nop
                                        Instr::Nop => {
                                            writer_tx.send(Write(conn.id, NOP.into())).unwrap();
                                        }

                                        // Set
                                        Instr::Set => {
                                            subs_tx
                                                .send(Call(key.to_owned(), value.to_owned()))
                                                .unwrap();

                                            data_tx.send(Set(key, value)).unwrap();
                                            writer_tx.send(Write(conn.id, OK.into())).unwrap();
                                        }

                                        // Set only if the key doesn't exists.
                                        Instr::SetIfNone => {
                                            data_tx.send(SetIfNone(key, value)).unwrap();
                                            writer_tx.send(Write(conn.id, OK.into())).unwrap();
                                        }

                                        // Makes the value an integer and increase it in 1.
                                        Instr::Inc => {
                                            data_tx.send(Inc(key, conn.id)).unwrap();
                                        }

                                        // Appends the value.
                                        Instr::Append => {
                                            data_tx.send(Append(key, value, conn.id)).unwrap();
                                        }

                                        // Delete!
                                        Instr::Delete => {
                                            data_tx.send(Delete(key)).unwrap();
                                            writer_tx.send(Write(conn.id, OK.into())).unwrap();
                                        }

                                        // Get
                                        Instr::Get => {
                                            data_tx.send(Get(key, conn.id)).unwrap();
                                        }

                                        // 0x0 separated key value enumeration: key value\0x0key2 value2
                                        Instr::KeyValue => {
                                            data_tx.send(Bite(key, conn.id)).unwrap();
                                        }

                                        // Trimmed Json (just the data).
                                        Instr::Jtrim => {
                                            data_tx.send(Jtrim(key, conn.id)).unwrap();
                                        }

                                        // Json (full path).
                                        Instr::Json => {
                                            data_tx.send(Json(key, conn.id)).unwrap();
                                        }

                                        // A generic "bite" subscription. Subscribers also receive their key: "key value"
                                        // Also a first message if value is available.
                                        Instr::SubGet | Instr::SubKeyValue | Instr::SubJson => {
                                            if !conn.keys.contains(&key) {
                                                conn.keys.push(key.to_owned());
                                            }

                                            subs_tx
                                                .send(Add(key.to_owned(), conn.id, instr))
                                                .unwrap();

                                            if !value.is_empty() {
                                                subs_tx.send(Call(key, value)).unwrap()
                                            }

                                            writer_tx.send(Write(conn.id, OK.into())).unwrap();
                                        }

                                        // A unsubscription and a last message if value is available.
                                        Instr::Unsub => {
                                            if !value.is_empty() {
                                                subs_tx.send(Call(key.to_owned(), value)).unwrap();
                                            }

                                            subs_tx.send(Del(key, conn.id)).unwrap();
                                            writer_tx.send(Write(conn.id, OK.into())).unwrap();
                                        }

                                        // Calls key subscribers with the new value without data modifications.
                                        Instr::SubCall => {
                                            subs_tx.send(Call(key, value)).unwrap();
                                            writer_tx.send(Write(conn.id, OK.into())).unwrap();
                                        }
                                    }

                                    println!("{}: {}", conn.addr, text);
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
                        self.writers.lock().unwrap().remove(&id).unwrap();
                        let rconn = self.readers.lock().unwrap().remove(&id).unwrap();
                        self.poller.delete(&rconn.socket).unwrap();
                        subs_tx.send(DelAll(rconn.keys, id)).unwrap();
                    }
                }
            }
        }
    }
}
