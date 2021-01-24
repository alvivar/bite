use chrono::Utc;
use rayon;
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
    Set,
    Get,
    Nop,
}

struct Client {
    socket: TcpStream,
    address: SocketAddr,
    process: Process,
}

struct Update {
    result: String,
    process: Process,
}

fn main() {
    let data = Arc::new(Mutex::new(BTreeMap::<String, String>::new()));
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();

    // Incoming connections

    let server = TcpListener::bind("127.0.0.1:1984").unwrap();
    server.set_nonblocking(true).unwrap();

    let (sender, receiver) = mpsc::channel::<Client>();

    thread::spawn(move || loop {
        if let Ok((mut socket, addr)) = server.accept() {
            let result_sender = sender.clone();
            pool.spawn(move || {
                let process = parse_message(&mut socket);
                result_sender
                    .send(Client {
                        socket: socket,
                        address: addr,
                        process: process.unwrap(),
                    })
                    .unwrap();
            });
        }
    });

    // The main thread is going to process the I/O operations, based on
    // responses from the Thread pool.

    loop {
        match receiver.recv() {
            Ok(client) => {
                let update = update_btreemap(client.process, data.clone());

                let mut socket = client.socket;
                socket.write(update.result.as_bytes()).unwrap();
                socket.write(&[0xA]).unwrap(); // End of line
                socket.flush().unwrap();

                let ip = client.address;
                let now = Utc::now().format("%Y-%m-%d %H:%M:%S");
                let i = update.process.instruction;
                let k = update.process.key;
                let mut v = update.process.value;

                let i = match i {
                    Instruction::Get => {
                        v = update.result;
                        "GET"
                    }
                    Instruction::Set => "SET",
                    Instruction::Nop => "NOP",
                };

                println!("{} | {} | {} {} {}", now, ip, i, k, v);
            }
            Err(_) => {
                panic!("receiver.recv() Failed!")
            }
        }
    }
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
        "get" => Instruction::Get,
        "set" => Instruction::Set,
        _ => Instruction::Nop,
    };

    Ok(Process {
        instruction,
        key: key.trim().to_owned(),
        value: val.trim().to_owned(),
    })
}

fn update_btreemap(process: Process, data: Arc<Mutex<BTreeMap<String, String>>>) -> Update {
    let mut map = data.lock().unwrap();
    match process.instruction {
        Instruction::Get => match map.get(&process.key) {
            Some(content) => Update {
                result: content.to_owned(),
                process: process,
            },
            None => Update {
                result: "".to_owned(),
                process: process,
            },
        },
        Instruction::Set => {
            map.insert(process.key.to_owned(), process.value.to_owned());

            Update {
                result: "OK".to_owned(),
                process: process,
            }
        }
        Instruction::Nop => Update {
            result: "NOP".to_owned(),
            process: process,
        },
    }
}
