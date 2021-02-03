pub struct Proc {
    pub instr: Instr,
    pub key: String,
    pub value: String,
}

pub enum Instr {
    Set,
    Get,
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

pub fn to_json(key: &str, value: &str) -> String {
    let split = key.split(".");

    let mut json = "@".to_owned();
    let template = "{ \"k\" : @ }";

    for k in split {
        let keyed = template.replace("k", k);
        json = json.replace("@", keyed.as_str());
    }

    json = json.replace("@", "\"@\"");
    json = json.replace("@", value);

    // @todo Value needs to be int and float eventually.

    return json;
}
