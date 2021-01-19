use std::collections::BTreeMap;
use std::io::{BufReader, Read};
use std::net::{TcpListener, TcpStream};

enum Instruction {
    SET,
    GET,
}

struct Process {
    instruction: Instruction,
    key: String,
    value: String,
}

fn main() -> std::io::Result<()> {
    let mut key_values: BTreeMap<String, String> = BTreeMap::new();

    let listener = TcpListener::bind("127.0.0.1:8888")?;
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let inst = parse_message(stream);
                let proc = get_process(inst);

                match proc {
                    Ok(proc) => {
                        let something = get_update_map(&mut key_values, proc);
                        println!("{:?}", something);
                    }
                    Err(e) => println!("{}", e),
                }
            }
            Err(e) => {
                println!("Somehow an error:\n{}\n", e);
            }
        }
    }

    Ok(())
}

fn get_update_map(map: &mut BTreeMap<String, String>, proc: Process) -> Result<String, ()> {
    match proc.instruction {
        Instruction::GET => Ok(map.entry(proc.key).or_insert(proc.value).to_string()),
        Instruction::SET => {
            let val = map.entry(proc.key.clone()).or_insert(proc.value.clone());
            val.clear();
            val.push_str(proc.value.as_str());
            Ok(val.to_string())
        }
    }
}

fn get_process(inst: (String, String, String)) -> Result<Process, String> {
    match (inst.0.as_str(), inst.1.as_str(), inst.2.as_str()) {
        ("set", k, v) => Result::Ok(Process {
            instruction: Instruction::SET,
            key: k.to_string(),
            value: v.to_string(),
        }),
        ("get", k, v) => Result::Ok(Process {
            instruction: Instruction::GET,
            key: k.to_string(),
            value: v.to_string(),
        }),
        _ => Result::Err(format!("Probably bad format.")),
    }
}

fn parse_message(mut stream: TcpStream) -> (String, String, String) {
    let mut reader = BufReader::new(&mut stream);
    let mut content = String::new();
    reader.read_to_string(&mut content);

    let mut inst = String::new();
    let mut key = String::new();
    let mut value = String::new();

    let mut found = 0;
    for c in content.chars() {
        match c {
            ' ' => {
                if value.len() > 0 {
                    value.push(' ');
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
                    value.push(c);
                }
            },
        }
    }

    (inst.to_lowercase(), key, value)
}
