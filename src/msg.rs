pub struct Msg {
    pub op: String,
    pub key: String,
    pub value: String,
}

/// Returns a Msg with the first character found as op, the next word as key,
/// and the rest as value.

/// This text: +hello world is a pretty old meme
/// Returns: Msg { "+", "hello", "world is a pretty old meme" }

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

                    // The firts non-space found is the (op)erator. Separated or
                    // not by space.
                    next = 1;
                }

                1 => {
                    key.push(c);
                }

                _ => {
                    // @doubt There may be a way to push the rest of the
                    // iterator instead of one by one.
                    value.push(c);
                }
            },
        }
    }

    let op = op.trim().to_owned();
    let key = key.trim().to_owned();

    Msg { op, key, value }
}
