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

pub fn parse(text: &str) -> Msg {
    let mut op = String::new();
    let mut key = String::new();
    let mut value = String::new();

    let mut word = 0;
    for c in text.chars() {
        match c {
            _ if c.is_whitespace() => {
                if !value.is_empty() {
                    value.push(' ');
                } else if !key.is_empty() {
                    word = 2;
                } else if !op.is_empty() {
                    word = 1;
                }
            }

            _ => match word {
                0 => op.push(c),
                1 => key.push(c),
                _ => value.push(c),
            },
        }
    }

    let instr = match op.trim_end().to_lowercase().as_str() {
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
