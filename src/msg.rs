use core::fmt::{Debug, Display, Formatter, Result};
use std::{io::Cursor, str::from_utf8};

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
    let op = from_utf8(next_word(&mut cursor)).unwrap();
    let key = from_utf8(next_word(&mut cursor)).unwrap();
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

pub fn next_line<'a>(src: &mut Cursor<&'a [u8]>) -> &'a [u8] {
    let mut start = src.position() as usize;
    let mut end = src.get_ref().len();

    if start >= end {
        return &[];
    }

    while src.get_ref()[start] == b' ' {
        start += 1;
    }

    for i in start..end {
        if src.get_ref()[i] == b'\n' {
            end = i;
            break;
        }
    }

    src.set_position(end as u64);

    &src.get_ref()[start..end]
}

fn next_word<'a>(src: &mut Cursor<&'a [u8]>) -> &'a [u8] {
    let mut start = src.position() as usize;
    let mut end = src.get_ref().len();

    if start >= end {
        return &[];
    }

    while src.get_ref()[start] == b' ' {
        start += 1;
    }

    for i in start..end {
        if src.get_ref()[i] == b' ' {
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

    while src.get_ref()[start] == b' ' {
        start += 1;
    }

    &src.get_ref()[start..end]
}
