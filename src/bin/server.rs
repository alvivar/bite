use chrono::Utc;
use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

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

struct Update {
    result: String,
    process: Process,
}

enum Message {
    Result(TcpStream, Process),
}

#[derive(Debug)]
enum Command {
    New(TcpStream),
}

struct Worker {
    join_handle: thread::JoinHandle<()>,
    sender: mpsc::Sender<Command>,
}

impl Worker {
    pub fn new(results: mpsc::Sender<Message>) -> Self {
        let (sender, receiver) = mpsc::channel::<Command>();
        let join_handle = thread::spawn(move || Self::worker(receiver, results));
        Self {
            join_handle,
            sender,
        }
    }

    pub fn command(&self, cmd: Command) {
        self.sender.send(cmd).unwrap();
    }

    fn worker(commands: mpsc::Receiver<Command>, results: mpsc::Sender<Message>) {
        let mut clients = Vec::<TcpStream>::new();

        loop {
            // Commands

            if let Ok(command) = commands.try_recv() {
                println!("Worker detected: {:?}", command);
                match command {
                    Command::New(socket) => {
                        clients.push(socket.try_clone().unwrap());
                    }
                }
            }

            // Parse the messages from the clients

            for i in 0..clients.len() {
                let mut socket = clients[i].try_clone().unwrap();
                socket.set_nonblocking(true).unwrap();

                let mut reader = BufReader::new(&mut socket);
                let mut content = String::new();
                match reader.read_line(&mut content) {
                    Ok(_) => {
                        let process = parse_message(&mut content).unwrap();
                        results
                            .send(Message::Result(clients[i].try_clone().unwrap(), process))
                            .unwrap();
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        // This is the kind of stuff we want to ignore
                    }
                    Err(e) => {
                        panic!("reader.read_line failed:\n{}", e);
                    }
                }
            }
        }
    }
}

fn main() {
    let data = Arc::new(Mutex::new(BTreeMap::<String, String>::new()));

    // Incoming connections

    let server = TcpListener::bind("127.0.0.1:1984").unwrap();
    server.set_nonblocking(true).unwrap();

    let (sender, receiver) = mpsc::channel::<Message>();

    let mut worker_index = 0;
    let mut workers = Vec::<Worker>::new();
    workers.push(Worker::new(sender.clone()));
    workers.push(Worker::new(sender.clone()));
    workers.push(Worker::new(sender.clone()));

    thread::spawn(move || loop {
        if let Ok((socket, address)) = server.accept() {
            workers[worker_index].command(Command::New(socket));
            worker_index = (worker_index + 1) % workers.len();

            println!("Connection {}", worker_index);
        }
    });

    // The main thread is going to process the I/O operations, based on
    // responses from the Thread pool.

    loop {
        match receiver.recv() {
            Ok(message) => {
                match message {
                    Message::Result(socket, process) => {
                        let update = update_btreemap(process, data.clone());

                        let mut socket = socket.try_clone().unwrap();
                        socket.write(update.result.as_bytes()).unwrap();
                        socket.write(&[0xA]).unwrap();
                        socket.flush().unwrap();

                        let ip = 0; //address;
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
                }
            }
            Err(e) => {
                panic!("receiver.recv() Failed:\n{}", e)
            }
        }
    }
}

fn parse_message(content: &mut String) -> std::io::Result<Process> {
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

    println!("i[{}] k[{}] v[{}]", inst, key, val); // Debug

    let instruction = match inst.trim().to_lowercase().as_str() {
        "get" => Instruction::Get,
        "set" => Instruction::Set,
        _ => {
            key = format!("{} {}", inst, key);
            Instruction::Nop
        }
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
                process,
            },
            None => Update {
                result: "".to_owned(),
                process,
            },
        },
        Instruction::Set => {
            map.insert(process.key.to_owned(), process.value.to_owned());

            Update {
                result: "OK".to_owned(),
                process,
            }
        }
        Instruction::Nop => Update {
            result: "NOP".to_owned(),
            process,
        },
    }
}
