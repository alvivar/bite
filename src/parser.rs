use crate::data;
use crate::data::Action::{Append, Delete, Get, Inc, Json, Jtrim, KeyValue, Set, SetIfNone};
use crate::message::Message;
use crate::subs;
use crate::subs::Action::{Add, Call, Del};
use crate::writer::Order;
use crate::writer::{self, Action::Queue};

use core::fmt::{Debug, Display, Formatter, Result};
use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::mpsc::{channel, Receiver, Sender};

const OK: &str = "OK";
const NO: &str = "NO";

pub enum Action {
    Parse(Message, SocketAddr),
}

pub struct Parsed {
    pub command: Command,
    pub key: String,
    pub data: Vec<u8>,
}

#[derive(PartialEq, Debug)]
pub enum Command {
    No,
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
    pub tx: Sender<Action>,
    rx: Receiver<Action>,
}

impl Parser {
    pub fn new() -> Parser {
        let (tx, rx) = channel::<Action>();

        Parser { tx, rx }
    }

    pub fn handle(
        &self,
        data_tx: Sender<data::Action>,
        writer_tx: Sender<writer::Action>,
        subs_tx: Sender<subs::Action>,
    ) {
        loop {
            match self.rx.recv().unwrap() {
                Action::Parse(message, addr) => {
                    let utf8 = String::from_utf8_lossy(&message.data);
                    let mut text = utf8.to_string();

                    let limit = 128;
                    if utf8.len() > limit {
                        text = truncate(&utf8, limit).into();
                        let add = format!(" (..{limit})");
                        text.push_str(&add);
                    };

                    info!("{addr} ({} bytes): {text}", message.data.len());

                    let from_id = message.from as usize;
                    let msg_id = message.id as usize;

                    let parsed = parse(&message.data);
                    let command = parsed.command;
                    let key = parsed.key;
                    let data = parsed.data;

                    match command {
                        // Commands that doesn't make sense without key.
                        _ if key.is_empty() && needs_key(&command) => {
                            writer_tx
                                .send(Queue(Order {
                                    from_id,
                                    to_id: from_id,
                                    msg_id,
                                    data: NO.into(),
                                }))
                                .unwrap();
                        }

                        // No
                        Command::No => {
                            writer_tx
                                .send(Queue(Order {
                                    from_id,
                                    to_id: from_id,
                                    msg_id,
                                    data: NO.into(),
                                }))
                                .unwrap();
                        }

                        // Set
                        Command::Set => {
                            writer_tx
                                .send(Queue(Order {
                                    from_id,
                                    to_id: from_id,
                                    msg_id,
                                    data: OK.into(),
                                }))
                                .unwrap();

                            subs_tx
                                .send(Call(key.to_owned(), data.to_owned(), from_id, msg_id))
                                .unwrap();

                            data_tx.send(Set(key, data)).unwrap();
                        }

                        // Set only if the key doesn't exists.
                        Command::SetIfNone => {
                            writer_tx
                                .send(Queue(Order {
                                    from_id,
                                    to_id: from_id,
                                    msg_id,
                                    data: OK.into(),
                                }))
                                .unwrap();

                            data_tx.send(SetIfNone(key, data, from_id, msg_id)).unwrap();
                        }

                        // Makes the value an integer and increase it in 1.
                        Command::Inc => {
                            data_tx.send(Inc(key, from_id, msg_id)).unwrap();
                        }

                        // Appends the value.
                        Command::Append => {
                            writer_tx
                                .send(Queue(Order {
                                    from_id,
                                    to_id: from_id,
                                    msg_id,
                                    data: OK.into(),
                                }))
                                .unwrap();

                            data_tx.send(Append(key, data, from_id, msg_id)).unwrap();
                        }

                        // Delete!
                        Command::Delete => {
                            writer_tx
                                .send(Queue(Order {
                                    from_id,
                                    to_id: from_id,
                                    msg_id,
                                    data: OK.into(),
                                }))
                                .unwrap();

                            data_tx.send(Delete(key)).unwrap();
                        }

                        // Get
                        Command::Get => {
                            data_tx.send(Get(key, from_id, msg_id)).unwrap();
                        }

                        // 0x0 separated key value enumeration: key value\0x0key2 value2
                        Command::KeyValue => {
                            data_tx.send(KeyValue(key, from_id, msg_id)).unwrap();
                        }

                        // Trimmed Json (just the data).
                        Command::Jtrim => {
                            data_tx.send(Jtrim(key, from_id, msg_id)).unwrap();
                        }

                        // Json (full path).
                        Command::Json => {
                            data_tx.send(Json(key, from_id, msg_id)).unwrap();
                        }

                        // A generic "bite" subscription. Subscribers also receive their key: "key value"
                        // Also a first message if value is available.
                        Command::SubGet | Command::SubKeyValue | Command::SubJson => {
                            writer_tx
                                .send(Queue(Order {
                                    from_id,
                                    to_id: from_id,
                                    msg_id,
                                    data: OK.into(),
                                }))
                                .unwrap();

                            subs_tx.send(Add(key.to_owned(), from_id, command)).unwrap();

                            if !data.is_empty() {
                                subs_tx.send(Call(key, data, from_id, msg_id)).unwrap()
                            }
                        }

                        // A unsubscription and a last message if value is available.
                        Command::Unsub => {
                            writer_tx
                                .send(Queue(Order {
                                    from_id,
                                    to_id: from_id,
                                    msg_id,
                                    data: OK.into(),
                                }))
                                .unwrap();

                            if !data.is_empty() {
                                subs_tx
                                    .send(Call(key.to_owned(), data, from_id, msg_id))
                                    .unwrap();
                            }

                            subs_tx.send(Del(key, from_id)).unwrap();
                        }

                        // Calls key subscribers with the new value without data modifications.
                        Command::SubCall => {
                            writer_tx
                                .send(Queue(Order {
                                    from_id,
                                    to_id: from_id,
                                    msg_id,
                                    data: OK.into(),
                                }))
                                .unwrap();

                            subs_tx.send(Call(key, data, from_id, msg_id)).unwrap();
                        }
                    }
                }
            }
        }
    }
}

/// Returns a Message with the first characters found as command, the next word
/// as key, and the rest as value.

/// This text: + hello world is a pretty old meme
/// Returns: Message { Command::Append, "hello", "world is a pretty old meme" }

pub fn parse(message: &[u8]) -> Parsed {
    let mut cursor = Cursor::new(message);
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
        _ => Command::No,
    };

    let key: String = key.trim_end().into();

    Parsed {
        command,
        key,
        data: data.into(),
    }
}

pub fn needs_key(command: &Command) -> bool {
    match command {
        Command::No | Command::KeyValue | Command::Jtrim | Command::Json => false,

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

#[allow(dead_code)]
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

    while is_space(src.get_ref()[start]) {
        start += 1;

        if start >= end {
            return &[];
        }
    }

    for i in start..end {
        if is_space(src.get_ref()[i]) {
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

    while is_space(src.get_ref()[start]) {
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

fn is_space(c: u8) -> bool {
    c == b' '
}

fn truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        None => s,
        Some((i, _)) => &s[..i],
    }
}
