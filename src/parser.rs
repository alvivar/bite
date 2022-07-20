use crate::data;
use crate::data::Cmd::{Append, Bite, Delete, Get, Inc, Json, Jtrim, Set, SetIfNone};
use crate::subs;
use crate::subs::Cmd::{Add, Call, Del};
use crate::writer::{self, Cmd::Push};

use crossbeam_channel::{unbounded, Receiver, Sender};

use core::fmt::{Debug, Display, Formatter, Result};
use std::io::Cursor;
use std::net::SocketAddr;
use std::str::from_utf8;

const OK: &str = "OK";
const NOP: &str = "NOP";
const KEY: &str = "KEY?";

pub enum Cmd {
    Parse(usize, Vec<u8>, SocketAddr),
}

pub struct Parser {
    pub tx: Sender<Cmd>,
    rx: Receiver<Cmd>,
}

impl Parser {
    pub fn new() -> Parser {
        let (tx, rx) = unbounded::<Cmd>();

        Parser { tx, rx }
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

pub struct Msg {
    pub instr: Instr,
    pub key: String,
    pub value: String,
}

#[derive(PartialEq, Debug)]
pub enum Instr {
    Nop,
    Set,
    SetIfNone,
    Inc,
    Append,
    Delete,
    Get,
    KeyValue,
    Jtrim,
    Json,
    SubGet,
    SubKeyValue,
    SubJson,
    Unsub,
    SubCall,
}

impl Display for Instr {
    fn fmt(&self, f: &mut Formatter) -> Result {
        Debug::fmt(self, f)
    }
}

/// Returns a Msg with the first character found as instruction,
/// the next word as key, and the rest as value.

/// This text: +hello world is a pretty old meme
/// Returns: Msg { Instr::Append, "hello", "world is a pretty old meme" }

pub fn parse(msg: &[u8]) -> Msg {
    let mut cursor = Cursor::new(msg);
    let op = to_utf8(next_word(&mut cursor));
    let key = to_utf8(next_word(&mut cursor));
    let value = from_utf8(remaining(&mut cursor)).unwrap();

    let instr = match op.to_lowercase().trim_end() {
        "s" => Instr::Set,
        "s?" => Instr::SetIfNone,
        "+1" => Instr::Inc,
        "+" => Instr::Append,
        "d" => Instr::Delete,
        "g" => Instr::Get,
        "k" => Instr::KeyValue,
        "j" => Instr::Jtrim,
        "js" => Instr::Json,
        "#g" => Instr::SubGet,
        "#k" => Instr::SubKeyValue,
        "#j" => Instr::SubJson,
        "#-" => Instr::Unsub,
        "!" => Instr::SubCall,
        _ => Instr::Nop,
    };

    // @todo In the future value needs to be a [u8] or at least a Vec<u8>.
    let key = key.trim_end().to_owned();
    let value = value.trim_end().to_owned();

    Msg { instr, key, value }
}

pub fn needs_key(instr: &Instr) -> bool {
    match instr {
        Instr::Nop | Instr::KeyValue | Instr::Jtrim | Instr::Json => false,

        Instr::Set
        | Instr::SetIfNone
        | Instr::Inc
        | Instr::Append
        | Instr::Delete
        | Instr::Get
        | Instr::SubGet
        | Instr::SubKeyValue
        | Instr::SubJson
        | Instr::Unsub
        | Instr::SubCall => true,
    }
}

pub fn next_line<'a>(cursor: &mut Cursor<&'a [u8]>) -> Option<&'a [u8]> {
    let mut start = cursor.position() as usize;
    let mut end = cursor.get_ref().len();

    if start >= end {
        return None;
    }

    while is_newline(cursor.get_ref()[start]) {
        start += 1;

        if start >= end {
            return None;
        }
    }

    for i in start..end {
        if is_newline(cursor.get_ref()[i]) {
            end = i;
            break;
        }
    }

    cursor.set_position(end as u64);

    Some(&cursor.get_ref()[start..end])
}

pub fn next_word<'a>(src: &mut Cursor<&'a [u8]>) -> &'a [u8] {
    let mut start = src.position() as usize;
    let mut end = src.get_ref().len();

    if start >= end {
        return &[];
    }

    while is_whitespace(src.get_ref()[start]) {
        start += 1;

        if start >= end {
            return &[];
        }
    }

    for i in start..end {
        if is_whitespace(src.get_ref()[i]) {
            end = i;
            break;
        }
    }

    src.set_position(end as u64);

    &src.get_ref()[start..end]
}

fn remaining<'a>(src: &mut Cursor<&'a [u8]>) -> &'a [u8] {
    let mut start = src.position() as usize;
    let end = src.get_ref().len();

    if start >= end {
        return &[];
    }

    while is_whitespace(src.get_ref()[start]) {
        start += 1;

        if start >= end {
            return &[];
        }
    }

    &src.get_ref()[start..end]
}

fn to_utf8(str: &[u8]) -> &str {
    match from_utf8(str) {
        Ok(str) => str,
        Err(_) => "",
    }
}

fn is_newline(c: u8) -> bool {
    c == b'\r' || c == b'\n'
}

fn is_whitespace(c: u8) -> bool {
    c == b' ' || c == b'\t' || c == b'\r' || c == b'\n'
}
