use chrono::Utc;
use rayon;
use serde::de::value;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::{collections::BTreeMap, net::SocketAddr};

struct Process {
    instruction: Instruction,
    key: String,
    value: String,
}

enum Instruction {
    SET,
    GET,
    NOP,
}

struct Package {
    socket: TcpStream,
    address: SocketAddr,
    process: Process,
}

fn main() {
    let data = Arc::new(Mutex::new(BTreeMap::<String, String>::new()));

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();

    // Incoming

    let server = TcpListener::bind("127.0.0.1:1984").unwrap();
    server.set_nonblocking(true).unwrap();

    let (sender, receiver) = mpsc::channel::<Package>();

    thread::spawn(move || loop {
        if let Ok((mut socket, addr)) = server.accept() {
            let result_sender = sender.clone();
            pool.spawn(move || {
                let process = parse_message(&mut socket);
                result_sender
                    .send(Package {
                        socket: socket,
                        address: addr,
                        process: process.unwrap(),
                    })
                    .unwrap();
            });
        }
    });

    loop {
        match receiver.recv() {
            Ok(p) => {
                update_map_worker_thread(p, data.clone());
            }
            Err(_) => {}
        }
    }

    // The main thread is going to process the I/O operations, based on
    // responses from clients messages.
}

fn parse_message(stream: &mut TcpStream) -> std::io::Result<Process> {
    let mut reader = BufReader::new(stream);
    let mut content = String::new();
    reader.read_line(&mut content)?;

    let mut inst = String::new();
    let mut key = String::new();
    let mut val = String::new();

    let mut found = 0;
    for c in content.chars() {
        match c {
            ' ' => {
                // Makes more sense in reverse.
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

    let instruction = match inst.trim().to_lowercase().as_str() {
        "get" => Instruction::GET,
        "set" => Instruction::SET,
        _ => Instruction::NOP,
    };

    Ok(Process {
        instruction,
        key: key.trim().to_owned(),
        value: val.trim().to_owned(),
    })
}

fn update_map_worker_thread(
    package: Package,
    data: Arc<Mutex<BTreeMap<String, String>>>,
) -> String {
    let mut map = data.lock().unwrap();
    match package.process.instruction {
        Instruction::GET => match map.get(&package.process.key) {
            Some(content) => {
                println!(
                    "{} GET {} {}",
                    Utc::now().format("%Y-%m-%d %H:%M:%S"),
                    package.process.key,
                    content
                );

                content.clone()
            }
            None => {
                println!(
                    "{} GET {}",
                    Utc::now().format("%Y-%m-%d %H:%M:%S"),
                    package.process.key
                );

                "".to_owned()
            }
        },
        Instruction::SET => {
            map.insert(
                package.process.key.to_owned(),
                package.process.value.to_owned(),
            );

            println!(
                "{} SET {} {}",
                Utc::now().format("%Y-%m-%d %H:%M:%S"),
                package.process.key,
                package.process.value.to_owned()
            );

            "OK".to_owned()
        }
        Instruction::NOP => {
            println!(
                "{} SET {} {}",
                Utc::now().format("%Y-%m-%d %H:%M:%S"),
                package.process.key,
                package.process.value.to_owned()
            );

            "".to_owned()
        }
    }
}
