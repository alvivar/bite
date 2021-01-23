use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, RwLock};
use std::{collections::BTreeMap, thread};
use std::{
    fs::write,
    io::{BufRead, BufReader, Write},
};

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
    let key_values: BTreeMap<String, String> = BTreeMap::new();
    let data = Arc::new(Mutex::new(key_values));
    let mut handles = vec![];

    let listener = TcpListener::bind("127.0.0.1:1984")?;
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let stream = Arc::new(RwLock::new(stream));
                let instruction = parse_message(stream.clone())?;
                let process = get_process(instruction);

                // Current one:
                // 1. [a] Lock the stream
                // 2. [a] Read a message
                // 3. Launch a thread that will process the message (expensive because launching a new thread is expensive)
                // 4. [a] Lock the stream within this thread
                // 5. [a] Send the reply within this thread
                //
                // Problems:
                // 1. Lots of lockign between [a]
                // 2. Launching a thread is expensive, we're doing it for every message

                // Better one:
                // 1. Thread pool for however many CPU cores you have to handle the messages
                // 2. An non-blocking message queue to feed messages to the thread pool
                // 3. A blocking message queue to feed results from the thread pool to main thread
                // 4. Only your main thread handles any I/O (I/O is slow)
                //
                // Benefits:
                // 1. The thread pool ..
                // 2. One thread handles IO and n threads handle messages as fast as your CPU can do on each core (no locking, ever)

                match process {
                    Ok(proc) => {
                        let key_values = Arc::clone(&data);
                        let handle = thread::spawn(move || {
                            let result = update_map_worker_thread(proc, key_values.clone());
                            save_map_to_disc(&key_values);
                            let mut stream = stream.write().unwrap();
                            stream.write(result.as_bytes()).unwrap();
                            stream.write(&[0xA]).unwrap(); // End of line \n
                            stream.flush().unwrap();
                        });
                        handles.push(handle);
                    }
                    Err(e) => println!("{}", e),
                }
            }
            Err(e) => {
                println!("Somehow a listener.incoming() error?\n{}\n", e);
            }
        }
    }

    for handle in handles {
        handle.join().unwrap();
    }

    Ok(())
}

fn save_map_to_disc(data: &Arc<Mutex<BTreeMap<String, String>>>) {
    let map = data.lock().unwrap();
    println!("btree: {:?}", serde_json::to_string(&*map).unwrap());
}

fn update_map_worker_thread(proc: Process, data: Arc<Mutex<BTreeMap<String, String>>>) -> String {
    let mut map = data.lock().unwrap();
    match proc.instruction {
        Instruction::GET => match map.get(&proc.key) {
            Some(content) => {
                println!(
                    "{} GET {} {}",
                    Utc::now().format("%Y-%m-%d %H:%M:%S"),
                    proc.key,
                    content
                );

                content.clone()
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
            map.insert(proc.key.to_owned(), proc.value.to_owned());

            println!(
                "{} SET {} {}",
                Utc::now().format("%Y-%m-%d %H:%M:%S"),
                proc.key,
                proc.value.to_owned()
            );

            "OK".to_owned()
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
            key: k.to_owned(),
            value: v.to_owned(),
        }),
        ("get", k, v) => Ok(Process {
            instruction: Instruction::GET,
            key: k.to_owned(),
            value: v.to_owned(),
        }),
        (i, k, v) => Err(format!(
            "{} NOP {} {} {}",
            Utc::now().format("%Y-%m-%d %H:%M:%S"),
            i.to_owned(),
            k.to_owned(),
            v.to_owned()
        )),
    }
}

fn parse_message(stream: Arc<RwLock<TcpStream>>) -> std::io::Result<(String, String, String)> {
    // @bot Clone makes sense here? It's the only way I know how to do it
    let mut stream = &*stream.write().unwrap();
    let mut reader = BufReader::new(&mut stream);
    let mut content = String::new();
    reader.read_line(&mut content).unwrap();

    let mut inst = String::new();
    let mut key = String::new();
    let mut val = String::new();

    let mut found = 0;
    for c in content.chars() {
        match c {
            ' ' => {
                // Makes more sense in rever order.
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

    Ok((
        inst.trim().to_lowercase(),
        key.trim().to_owned(),
        val.trim().to_owned(),
    ))
}
