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
    Get,
    Bite,
    Jtrim,
    Json,
    Delete,
    SubJ,
    SubGet,
    SubBite,
    Signal,
    Unsub,
}

/// Returns a Msg with the first character found as instruction, the next word
/// as key, and the rest as value.

/// This text: +hello world is a pretty old meme
/// Returns: Msg { Instr::Append, "hello", "world is a pretty old meme" }

pub fn parse(text: &str) -> Msg {
    let mut op = String::new();
    let mut key = String::new();
    let mut value = String::new();

    let mut next = 0;
    for c in text.chars() {
        match c {
            ' ' => {
                if !value.is_empty() {
                    value.push(' ');
                } else if !key.is_empty() {
                    next = 2;
                } else if !op.is_empty() {
                    next = 1;
                }
            }

            _ => match next {
                0 => {
                    op.push(c);
                }

                1 => {
                    key.push(c);
                }

                _ => {
                    // @todo There may be a way to push the rest of the iterator
                    // instead of one by one.
                    value.push(c);
                }
            },
        }
    }

    let instr = match op.trim().to_lowercase().as_str() {
        "s" => Instr::Set,
        "s?" => Instr::SetIfNone,
        "+1" => Instr::Inc,
        "+" => Instr::Append,
        "g" => Instr::Get,
        "b" => Instr::Bite,
        "j" => Instr::Jtrim,
        "js" => Instr::Json,
        "d" => Instr::Delete,
        "#j" => Instr::SubJ,
        "#g" => Instr::SubGet,
        "#b" => Instr::SubBite,
        "!" => Instr::Signal,
        "-#" => Instr::Unsub,
        _ => Instr::Nop,
    };

    let key = key.trim().to_owned();

    Msg { instr, key, value }
}
