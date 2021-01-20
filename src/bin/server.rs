use chrono::Utc;
use std::io::{BufReader, Read};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, RwLock};
use std::{collections::BTreeMap, thread};

struct Process {
    instruction: Instruction,
    key: String,
    value: String,
}

enum Instruction {
    SET,
    GET,
}

fn main() -> std::io::Result<()> {
    let key_values: BTreeMap<String, Mutex<String>> = BTreeMap::new();
    let data = Arc::new(RwLock::new(key_values));
    let mut handles = vec![];

    let listener = TcpListener::bind("127.0.0.1:1984")?;
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let instruction = parse_message(stream)?;
                let process = get_process(instruction);

                match process {
                    Ok(proc) => {
                        let key_values = Arc::clone(&data);
                        let handle = thread::spawn(move || {
                            update_map_worker_thread(proc, key_values);
                        });
                        handles.push(handle);
                    }
                    Err(e) => println!("{}", e),
                }
            }
            Err(e) => {
                println!("Somehow an error:\n{}\n", e);
            }
        }
    }

    for handle in handles {
        handle.join().unwrap();
    }

    Ok(())
}

fn update_map_worker_thread(
    proc: Process,
    data: Arc<RwLock<BTreeMap<String, Mutex<String>>>>,
) -> String {
    let mut map = data.write().unwrap();
    match proc.instruction {
        Instruction::GET => match map.get(&proc.key) {
            Some(d) => {
                let mutex = d.lock().unwrap();
                let content = mutex.to_owned();

                println!(
                    "{} GET {} {}",
                    Utc::now().format("%Y-%m-%d %H:%M:%S"),
                    proc.key,
                    content
                );

                content
            }
            None => {
                println!(
                    "{} GET {}",
                    Utc::now().format("%Y-%m-%d %H:%M:%S"),
                    proc.key
                );

                "".to_owned()
            }
        },
        Instruction::SET => {
            let data = map
                .entry(proc.key.to_owned())
                .or_insert(Mutex::new(proc.value.to_owned()));

            *data.lock().unwrap() = proc.value.to_owned();

            let mutex = data.lock().unwrap();
            let content = mutex.to_owned();

            println!(
                "{} SET {} {}",
                Utc::now().format("%Y-%m-%d %H:%M:%S"),
                proc.key,
                content
            );

            content
        }
    }
}

// @bot Before threads.
// fn get_update_map(map: &mut BTreeMap<String, String>, proc: Process) -> Result<String, ()> {
//     match proc.instruction {
//         Instruction::GET => Ok(map.entry(proc.key).or_insert(proc.value).to_string()),
//         Instruction::SET => {
//             let val = map.entry(proc.key.clone()).or_insert(proc.value.clone());
//             val.clear();
//             val.push_str(proc.value.as_str());
//             Ok(val.to_string())
//         }
//     }
// }

fn get_process(inst: (String, String, String)) -> Result<Process, String> {
    match (inst.0.as_str(), inst.1.as_str(), inst.2.as_str()) {
        ("set", k, v) => Ok(Process {
            instruction: Instruction::SET,
            key: k.to_string(),
            value: v.to_string(),
        }),
        ("get", k, v) => Ok(Process {
            instruction: Instruction::GET,
            key: k.to_string(),
            value: v.to_string(),
        }),
        _ => Err(format!("{} NOP", Utc::now().format("%Y-%m-%d %H:%M:%S"))),
    }
}

fn parse_message(mut stream: TcpStream) -> std::io::Result<(String, String, String)> {
    let mut reader = BufReader::new(&mut stream);
    let mut content = String::new();
    reader.read_to_string(&mut content)?;

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

    Ok((inst.to_lowercase(), key, value))
}
