use std::io::{BufRead, BufReader, Read};
use std::net::{TcpListener, TcpStream};
use std::{
    collections::{btree_map::Values, BTreeMap},
    string,
};

enum Instruction {
    NOP,
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
                let inst = parse_instruction(stream)?;
                let proc = get_process(inst);

                let something = insert_into_map(&mut key_values, proc);
                println!("{}", something);
            }
            Err(e) => {
                println!("Somehow an error:\n{}\n", e);
            }
        }
    }

    Ok(())
}

fn insert_into_map(map: &mut BTreeMap<String, String>, proc: Process) -> String {
    let mut value_inserted = String::new();
    match proc.instruction {
        Instruction::GET => {
            value_inserted.push_str(map.get(&proc.key).unwrap().as_str());
        }
        Instruction::SET => {
            value_inserted.push_str(map.entry(proc.key).or_insert(proc.value).as_str());
        }
        Instruction::NOP => {}
    }

    value_inserted
}

fn get_process(instructions: (String, String, String)) -> Process {
    match (
        instructions.0.as_str(),
        instructions.1.as_str(),
        instructions.2.as_str(),
    ) {
        ("set", k, v) => Process {
            instruction: Instruction::SET,
            key: k.to_string(),
            value: v.to_string(),
        },
        ("get", k, v) => Process {
            instruction: Instruction::GET,
            key: k.to_string(),
            value: v.to_string(),
        },
        _ => Process {
            instruction: Instruction::NOP,
            key: String::new(),
            value: String::new(),
        },
    }
}

fn parse_instruction(mut stream: TcpStream) -> std::io::Result<(String, String, String)> {
    let mut reader = BufReader::new(&mut stream);
    let mut content = String::new();
    reader.read_to_string(&mut content)?;

    let mut instruction = String::new();
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
                } else if instruction.len() > 0 {
                    found = 1;
                }
            }
            _ => match found {
                0 => {
                    instruction.push(c);
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

    Ok((instruction, key, value))
}
