use std::{env::consts::FAMILY, mem, usize};

use serde_json::{json, map::Entry, Value};

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
    let mut entry: Entry;

    for mut k in key.split('.') {
        let is_list = is_klist(k);

        let mut kli = String::new();
        if is_list {
            let mut split = k.split("[");
            k = split.next().unwrap();
            kli = split.next().unwrap().split("]").next().unwrap().to_string();
            println!("{}", kli);
        }

        match json {
            Value::Array(_) => {
                // let map = json
                //     .as_array()
                //     .unwrap()
                //     .iter()
                //     .find(|x| x.as_object().unwrap().contains_key(key))
                //     .unwrap();

                // println!("Map {}", &map);

                println!("{} {} {} ", kli, key, val);

                // let arr = json.as_array_mut().unwrap();
                // arr.push(json!({ k: val }));

                // let arr = json.as_array_mut().unwrap();
                // if arr.len() < 1 {
                //     arr.push(json!({ k: val }));
                // } else {
                //     println!("kli {}", kli);
                //     mem::replace(&mut arr[kli], json!({ k: val }));
                // }
            }
            Value::Object(_) => {
                entry = json.as_object_mut().unwrap().entry(k);

                match is_list {
                    true => json = entry.or_insert_with(|| json!([])),
                    false => json = entry.or_insert_with(|| json!({})),
                }
            }
            _ => {}
        }
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

fn is_klist(str: &str) -> bool {
    let left = str.contains("[");
    let right = str.contains("]");

    if !left || !right {
        return false;
    }

    true
}
