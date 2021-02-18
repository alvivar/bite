use serde_json::{json, Value};

pub struct Proc {
    pub instr: Instr,
    pub key: String,
    pub value: String,
}

pub enum Instr {
    Set,
    Get,
    Json,
    Jtrim,
    SubJtrim,
    Nop,
}

pub enum AsyncInstr {
    Yes,
    No(String),
}

pub fn proc_from_string(content: &str) -> Proc {
    let mut inst = String::new();
    let mut key = String::new();
    let mut val = String::new();

    let mut found = 0;
    for c in content.trim().chars() {
        match c {
            ' ' => {
                if val.len() > 0 {
                    val.push(' ');
                } else if key.len() > 0 {
                    found = 2;
                } else if inst.len() > 0 {
                    found = 1;
                }
            }
            _ => match found {
                0 => {
                    inst.push(c);
                }
                1 => {
                    key.push(c);
                }
                _ => {
                    val.push(c);
                }
            },
        }
    }

    // println!("i[{}] k[{}] v[{}]", inst, key, val); // Debug

    let instruction = match inst.trim().to_lowercase().as_str() {
        "g" => Instr::Get,
        "s" => Instr::Set,
        "js" => Instr::Json,
        "j" => Instr::Jtrim,
        "#j" => Instr::SubJtrim,
        _ => {
            key = format!("{} {}", inst, key);
            Instr::Nop
        }
    };

    Proc {
        instr: instruction,
        key: key.trim().to_owned(),
        value: val.trim().to_owned(),
    }
}

pub fn kv_to_json(kv: &[(&str, &str)]) -> Value {
    let mut merged_json = json!({});

    // NOTE(Wojciech): Unfinised alternative.
    // kv.iter().map(|(k, v)| k.split(".").map(|name| {}));

    for (k, v) in kv.iter().rev() {
        insert(&mut merged_json, k, json!(v));
    }

    return merged_json;
}

fn insert(mut json: &mut Value, key: &str, val: Value) {
    for k in key.split('.') {
        json = json
            .as_object_mut()
            .unwrap()
            .entry(k)
            .or_insert_with(|| json!({}));
    }

    if **&json == json!({}) {
        *json = val;
    }
}
