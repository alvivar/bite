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
    Nop,
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
        "get" => Instr::Get,
        "set" => Instr::Set,
        "json" => Instr::Json,
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

pub fn kv_to_json(kv: Vec<(&String, &String)>) -> Value {
    let mut merged_json = json!({});

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

    let inside = &json;

    match **inside {
        Value::Array(_) => (*json).as_array_mut().unwrap().push(val),
        Value::Object(_) => {
            if **inside == json!({}) {
                *json = val;
            }
        }
        _ => {}
    }
}
