use crate::data;
use crate::data::Cmd::{Append, Bite, Delete, Get, Inc, Json, Jtrim, Set, SetIfNone};
use crate::subs;
use crate::subs::Cmd::{Add, Call, Del};
use crate::writer::{self, Cmd::Queue};

use crossbeam_channel::{unbounded, Receiver, Sender};

use core::fmt::{Debug, Display, Formatter, Result};
use std::io::Cursor;
use std::net::SocketAddr;

const OK: &str = "OK";
const NOP: &str = "NOP";
const KEY: &str = "KEY?";

pub enum Cmd {
    Parse(usize, Vec<u8>, SocketAddr),
}

pub struct Message {
    pub command: Command,
    pub key: String,
    pub data: Vec<u8>,
}

#[derive(PartialEq, Debug)]
pub enum Command {
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

impl Display for Command {
    fn fmt(&self, f: &mut Formatter) -> Result {
        Debug::fmt(self, f)
    }
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
                Cmd::Parse(id, data, addr) => {
                    // We assume multiple commands separated with newlines.
                    let mut cursor = Cursor::new(&data[..]);
                    while let Some(line) = next_line(&mut cursor) {
                        let message = parse(line);
                        let command = message.command;
                        let key = message.key;
                        let data = message.data;

                        println!("\n{}: {} {} {:?}", addr, command, key, data);

                        match command {
                            // Commands that doesn't make sense without key.
                            _ if key.is_empty() && needs_key(&command) => {
                                writer_tx.send(Queue(id, KEY.into())).unwrap();
                            }

                            // Nop
                            Command::Nop => {
                                writer_tx.send(Queue(id, NOP.into())).unwrap();
                            }

                            // Set
                            Command::Set => {
                                writer_tx.send(Queue(id, OK.into())).unwrap();
                                subs_tx.send(Call(key.to_owned(), data.to_owned())).unwrap();
                                data_tx.send(Set(key, data)).unwrap();
                            }

                            // Set only if the key doesn't exists.
                            Command::SetIfNone => {
                                writer_tx.send(Queue(id, OK.into())).unwrap();
                                data_tx.send(SetIfNone(key, data)).unwrap();
                            }

                            // Makes the value an integer and increase it in 1.
                            Command::Inc => {
                                data_tx.send(Inc(key, id)).unwrap();
                            }

                            // Appends the value.
                            Command::Append => {
                                data_tx.send(Append(key, data, id)).unwrap();
                            }

                            // Delete!
                            Command::Delete => {
                                writer_tx.send(Queue(id, OK.into())).unwrap();
                                data_tx.send(Delete(key)).unwrap();
                            }

                            // Get
                            Command::Get => {
                                data_tx.send(Get(key, id)).unwrap();
                            }

                            // 0x0 separated key value enumeration: key value\0x0key2 value2
                            Command::KeyValue => {
                                data_tx.send(Bite(key, id)).unwrap();
                            }

                            // Trimmed Json (just the data).
                            Command::Jtrim => {
                                data_tx.send(Jtrim(key, id)).unwrap();
                            }

                            // Json (full path).
                            Command::Json => {
                                data_tx.send(Json(key, id)).unwrap();
                            }

                            // A generic "bite" subscription. Subscribers also receive their key: "key value"
                            // Also a first message if value is available.
                            Command::SubGet | Command::SubKeyValue | Command::SubJson => {
                                writer_tx.send(Queue(id, OK.into())).unwrap();

                                subs_tx.send(Add(key.to_owned(), id, command)).unwrap();

                                if !data.is_empty() {
                                    subs_tx.send(Call(key, data)).unwrap()
                                }
                            }

                            // A unsubscription and a last message if value is available.
                            Command::Unsub => {
                                writer_tx.send(Queue(id, OK.into())).unwrap();

                                if !data.is_empty() {
                                    subs_tx.send(Call(key.to_owned(), data)).unwrap();
                                }

                                subs_tx.send(Del(key, id)).unwrap();
                            }

                            // Calls key subscribers with the new value without data modifications.
                            Command::SubCall => {
                                writer_tx.send(Queue(id, OK.into())).unwrap();
                                subs_tx.send(Call(key, data)).unwrap();
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Returns a Msg with the first character found as command,
/// the next word as key, and the rest as value.

/// This text: + hello world is a pretty old meme
/// Returns: Msg { Command::Append, "hello", "world is a pretty old meme" }

pub fn parse(msg: &[u8]) -> Message {
    let mut cursor = Cursor::new(msg);
    let instruction = String::from_utf8_lossy(next_word(&mut cursor));
    let key = String::from_utf8_lossy(next_word(&mut cursor));
    let data = remaining(&mut cursor);

    let command = match instruction.to_lowercase().trim_end() {
        "s" => Command::Set,
        "s?" => Command::SetIfNone,
        "+1" => Command::Inc,
        "+" => Command::Append,
        "d" => Command::Delete,
        "g" => Command::Get,
        "k" => Command::KeyValue,
        "j" => Command::Jtrim,
        "js" => Command::Json,
        "#g" => Command::SubGet,
        "#k" => Command::SubKeyValue,
        "#j" => Command::SubJson,
        "#-" => Command::Unsub,
        "!" => Command::SubCall,
        _ => Command::Nop,
    };

    let key = key.trim_end().to_owned();

    Message {
        command,
        key,
        data: data.to_owned(),
    }
}

pub fn needs_key(command: &Command) -> bool {
    match command {
        Command::Nop | Command::KeyValue | Command::Jtrim | Command::Json => false,

        Command::Set
        | Command::SetIfNone
        | Command::Inc
        | Command::Append
        | Command::Delete
        | Command::Get
        | Command::SubGet
        | Command::SubKeyValue
        | Command::SubJson
        | Command::Unsub
        | Command::SubCall => true,
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

fn is_newline(c: u8) -> bool {
    c == b'\r' || c == b'\n'
}

fn is_whitespace(c: u8) -> bool {
    c == b' ' || c == b'\t' || c == b'\r' || c == b'\n'
}
