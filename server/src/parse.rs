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

// pub fn to_json_old(key: &str, value: &str) -> String {
//     let split = key.split(".");

//     let mut json = "@".to_owned();
//     let template = "{ \"k\" : @ }";

//     for k in split {
//         let keyed = template.replace("k", k);
//         json = json.replace("@", keyed.as_str());
//     }

//     json = json.replace("@", "\"@\"");
//     json = json.replace("@", value);

//     // @todo Value needs to be int and float eventually.

//     return json;
// }

// pub fn key_to_json_template(key: &str) -> String {
//     let split = key.split(".");

//     let mut json = "@".to_owned();
//     let template = "{ \"k\" : @ }";

//     for k in split {
//         let keyed = template.replace("k", k);
//         json = json.replace("@", keyed.as_str());
//     }

//     return json;
// }

pub fn kv_to_json_value(kv: Vec<(&String, &String)>) -> String {
    let mut json: Value = json!(Value::Null);

    for (k, v) in kv.iter().rev() {
        let ks: Vec<&str> = k.split(".").collect();
        match ks.len() {
            1 => json[ks[0]] = json!(v),
            2 => json[ks[0]][ks[1]] = json!(v),
            3 => json[ks[0]][ks[1]][ks[2]] = json!(v),
            4 => json[ks[0]][ks[1]][ks[2]][ks[3]] = json!(v),
            5 => json[ks[0]][ks[1]][ks[2]][ks[3]][ks[4]] = json!(v),
            6 => json[ks[0]][ks[1]][ks[2]][ks[3]][ks[4]][ks[5]] = json!(v),
            7 => json[ks[0]][ks[1]][ks[2]][ks[3]][ks[4]][ks[5]][ks[6]] = json!(v),
            8 => json[ks[0]][ks[1]][ks[2]][ks[3]][ks[4]][ks[5]][ks[6]][ks[7]] = json!(v),
            9 => json[ks[0]][ks[1]][ks[2]][ks[3]][ks[4]][ks[5]][ks[6]][ks[7]][ks[8]] = json!(v),
            _ => (),
        }
    }

    return json.to_string();
}

// pub fn inside(mut json: Value, mut kv: Vec<&str>) -> Value {
//     match kv.len() <= 0 {
//         true => Value::Null,
//         false => {
//             json[kv[0]] = json.clone();
//             kv.drain(0..0);
//             inside(json, kv)
//         }
//     }
// }

// pub fn mapjson(ks: Vec<&str>, val: &str) -> Value {
//     let mut result: Value = Value::Null;

//     for k in ks.iter().rev() {
//         println!("{}", k);

//         if result == Value::Null {
//             result = json!(val);
//         }

//         let mut temp = Value::Null;
//         temp[k] = result;
//         println!("{}\n", temp);

//         result = temp;
//     }

//     result
// }
