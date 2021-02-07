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

    if **&json == json!({}) {
        *json = val;
    }
}

pub fn compact(mut dst_json: &mut Value, src_json: &Value) {
    match src_json {
        Value::Object(map) => {
            for (k, v) in map {
                if k == "0" {
                    println!("{}:{}\n", k, v);
                }

                let mut approved = false;
                let mut list: Vec<Value> = Vec::new();

                match v {
                    Value::Object(map2) => {
                        for (k2, v2) in map2 {
                            list.push(v2.clone());
                            if k2 == "0" {
                                approved = true;
                                println!("Child has 0");
                            }
                        }
                    }
                    _ => {}
                }

                let jlist = json!(list);
                if approved {
                    dst_json[k] = jlist.clone();
                    compact(&mut dst_json[k], &jlist);
                } else {
                    dst_json[k] = v.clone();
                    compact(&mut dst_json[k], v);
                }
            }
        }
        _ => {}
    }
}

pub fn deep_keys(value: &Value, current_path: Vec<String>, output: &mut Vec<Vec<String>>) {
    if current_path.len() > 0 {
        output.push(current_path.clone());
    }

    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let mut new_path = current_path.clone();
                new_path.push(k.to_owned());
                deep_keys(v, new_path, output);
            }
        }
        Value::Array(array) => {
            for (i, v) in array.iter().enumerate() {
                let mut new_path = current_path.clone();
                new_path.push(i.to_string().to_owned());
                deep_keys(v, new_path, output);
            }
        }
        _ => (),
    }
}
