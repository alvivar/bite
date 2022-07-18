use crate::data;
use crate::data::Cmd::{Append, Bite, Delete, Get, Inc, Json, Jtrim, Set, SetIfNone};
use crate::parse::{needs_key, next_line, parse, Instr};
use crate::subs;
use crate::subs::Cmd::{Add, Call, Del};
use crate::writer::{self, Cmd::Push};

use crossbeam_channel::{unbounded, Receiver, Sender};

use std::io::Cursor;
use std::net::SocketAddr;

const OK: &str = "OK";
const NOP: &str = "NOP";
const KEY: &str = "KEY?";

pub enum Cmd {
    Parse(usize, Vec<u8>, SocketAddr),
}

pub struct Frame {
    pub tx: Sender<Cmd>,
    rx: Receiver<Cmd>,
}

impl Frame {
    pub fn new() -> Frame {
        let (tx, rx) = unbounded::<Cmd>();

        Frame { tx, rx }
    }

    pub fn handle(
        &self,
        data_tx: Sender<data::Cmd>,
        writer_tx: Sender<writer::Cmd>,
        subs_tx: Sender<subs::Cmd>,
    ) {
        loop {
            match self.rx.recv().unwrap() {
                Cmd::Parse(id, message, addr) => {
                    let received_utf8 = String::from_utf8_lossy(&message);
                    println!("\nReceived: {}", received_utf8);

                    // We assume multiple instructions separated with newlines.
                    let mut cursor = Cursor::new(&message[..]);
                    while let Some(line) = next_line(&mut cursor) {
                        let msg = parse(line);
                        let instr = msg.instr;
                        let key = msg.key;
                        let value = msg.value;

                        println!("\n{}: {} {} {}", addr, instr, key, value);

                        match instr {
                            // Instructions that doesn't make sense without key.
                            _ if key.is_empty() && needs_key(&instr) => {
                                writer_tx.send(Push(id, KEY.into())).unwrap();
                            }

                            // Nop
                            Instr::Nop => {
                                writer_tx.send(Push(id, NOP.into())).unwrap();
                            }

                            // Set
                            Instr::Set => {
                                subs_tx
                                    .send(Call(key.to_owned(), value.to_owned()))
                                    .unwrap();

                                data_tx.send(Set(key, value)).unwrap();
                                writer_tx.send(Push(id, OK.into())).unwrap();
                            }

                            // Set only if the key doesn't exists.
                            Instr::SetIfNone => {
                                data_tx.send(SetIfNone(key, value)).unwrap();
                                writer_tx.send(Push(id, OK.into())).unwrap();
                            }

                            // Makes the value an integer and increase it in 1.
                            Instr::Inc => {
                                data_tx.send(Inc(key, id)).unwrap();
                            }

                            // Appends the value.
                            Instr::Append => {
                                data_tx.send(Append(key, value, id)).unwrap();
                            }

                            // Delete!
                            Instr::Delete => {
                                data_tx.send(Delete(key)).unwrap();
                                writer_tx.send(Push(id, OK.into())).unwrap();
                            }

                            // Get
                            Instr::Get => {
                                data_tx.send(Get(key, id)).unwrap();
                            }

                            // 0x0 separated key value enumeration: key value\0x0key2 value2
                            Instr::KeyValue => {
                                data_tx.send(Bite(key, id)).unwrap();
                            }

                            // Trimmed Json (just the data).
                            Instr::Jtrim => {
                                data_tx.send(Jtrim(key, id)).unwrap();
                            }

                            // Json (full path).
                            Instr::Json => {
                                data_tx.send(Json(key, id)).unwrap();
                            }

                            // A generic "bite" subscription. Subscribers also receive their key: "key value"
                            // Also a first message if value is available.
                            Instr::SubGet | Instr::SubKeyValue | Instr::SubJson => {
                                // @todo This validation below should happen, probably is subs_tx responsability.

                                // if !keys.contains(&key) {
                                //     keys.push(key.to_owned());
                                // }

                                subs_tx.send(Add(key.to_owned(), id, instr)).unwrap();

                                if !value.is_empty() {
                                    subs_tx.send(Call(key, value)).unwrap()
                                }

                                writer_tx.send(Push(id, OK.into())).unwrap();
                            }

                            // A unsubscription and a last message if value is available.
                            Instr::Unsub => {
                                if !value.is_empty() {
                                    subs_tx.send(Call(key.to_owned(), value)).unwrap();
                                }

                                subs_tx.send(Del(key, id)).unwrap();
                                writer_tx.send(Push(id, OK.into())).unwrap();
                            }

                            // Calls key subscribers with the new value without data modifications.
                            Instr::SubCall => {
                                subs_tx.send(Call(key, value)).unwrap();
                                writer_tx.send(Push(id, OK.into())).unwrap();
                            }
                        }
                    }
                }
            }
        }
    }
}
