use std::{io::Cursor, str::from_utf8};

pub struct Msg {
    pub instr: Instr,
    pub key: String,
    pub value: String,
}

#[derive(PartialEq)]
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

/// Returns a Msg with the first character found as instruction,
/// the next word as key, and the rest as value.

/// This text: +hello world is a pretty old meme
/// Returns: Msg { Instr::Append, "hello", "world is a pretty old meme" }

pub fn parse(msg: &[u8]) -> Msg {
    let mut cursor = Cursor::new(msg);
    let op = from_utf8(find(&mut cursor, b' ')).unwrap();
    let key = from_utf8(find(&mut cursor, b' ')).unwrap();
    let value = from_utf8(remaining(&mut cursor)).unwrap();

    println!("Parse: {}| {}| {}|", op.trim(), key.trim(), value.trim());

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

fn find<'a>(src: &mut Cursor<&'a [u8]>, char: u8) -> &'a [u8] {
    let start = src.position() as usize;
    let end = src.get_ref().len() - 1;

    for i in start..end {
        if src.get_ref()[i] == char {
            src.set_position(i as u64);

            return &src.get_ref()[start..i];
        }
    }

    &[]
}

fn remaining<'a>(src: &mut Cursor<&'a [u8]>) -> &'a [u8] {
    let start = src.position() as usize;
    let end = src.get_ref().len() - 1;

    &src.get_ref()[start..end]
}
