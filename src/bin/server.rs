use chrono::Utc;
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

const DB_FILE: &str = "DB.json";
const UTC_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

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
    Result(Client, Process),
}

enum Command {
    New(TcpStream, SocketAddr),
}

struct Client {
    socket: TcpStream,
    addr: SocketAddr,
}

// struct WorkerPool {
//     workers: Mutex<Vec<Worker>>,
// }

// impl WorkerPool {
//     pub fn new() -> WorkerPool {
//         WorkerPool {
//             workers: Mutex::new(Vec::new()),
//         }
//     }

//     pub fn spawn(&self, worker: Worker) {
//         self.workers.lock().unwrap().push(worker);
//     }
// }

struct Worker {
    handle: thread::JoinHandle<()>,
    sender: mpsc::Sender<Command>,
}

impl Worker {
    pub fn new(result: mpsc::Sender<Message>) -> Self {
        let (sender, receiver) = mpsc::channel::<Command>();
        let handle = thread::spawn(move || Self::worker(receiver, result));
        Self { handle, sender }
    }

    pub fn command(&self, cmd: Command) {
        self.sender.send(cmd).unwrap();
    }

    fn worker(command: mpsc::Receiver<Command>, result: mpsc::Sender<Message>) {
        let mut clients = Vec::<Client>::new();

        loop {
            // Commands

            if let Ok(command) = command.try_recv() {
                match command {
                    Command::New(socket, addr) => {
                        println!("{} | {} | Connected", Utc::now().format(UTC_FORMAT), addr);
                        clients.push(Client { socket, addr });
                    }
                }
            }

            // Instructions parse

            for i in 0..clients.len() {
                // @ Non blocking read_line
                let mut socket = clients[i].socket.try_clone().unwrap();
                socket.set_nonblocking(true).unwrap();
                let addr = clients[i].addr;

                let mut reader = BufReader::new(&mut socket);
                let mut content = String::new();
                match reader.read_line(&mut content) {
                    Ok(_) => {
                        // @ TcpStream disconnection
                        if content.len() <= 0 {
                            clients.remove(i);
                            println!(
                                "{} | {} | Disconnected",
                                Utc::now().format(UTC_FORMAT),
                                addr
                            );
                            continue;
                        }

                        let process = parse_message(&mut content).unwrap();

                        result
                            .send(Message::Result(
                                Client {
                                    socket: socket.try_clone().unwrap(),
                                    addr,
                                },
                                process,
                            ))
                            .unwrap();
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
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

    // Read the DB file

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(DB_FILE);

    let mut contents = String::new();
    file.unwrap().read_to_string(&mut contents).unwrap();
    if contents.len() > 0 {
        let c = data.clone();
        let mut map = c.lock().unwrap();
        *map = serde_json::from_str(&contents).unwrap();
    }

    // New worker on incoming connections

    let server = TcpListener::bind("127.0.0.1:1984").unwrap();
    server.set_nonblocking(true).unwrap();

    // ?
    // let pool = WorkerPool::new();
    // pool.spawn(Worker::new(sender.clone()));
    // pool.spawn(Worker::new(sender.clone()));

    let (sender, result) = mpsc::channel::<Message>();
    let workers = [Worker::new(sender.clone()), Worker::new(sender.clone())];
    let mut indx = 0;

    thread::spawn(move || loop {
        if let Ok((socket, addr)) = server.accept() {
            workers[indx].command(Command::New(socket, addr));
            indx = (indx + 1) % workers.len();
        }
    });

    // The main thread processes the I/O, waiting from Workers messages.

    loop {
        match result.recv() {
            Ok(message) => {
                match message {
                    Message::Result(client, process) => {
                        let mut socket = client.socket.try_clone().unwrap();

                        // Map update

                        let update = update_btreemap(process, data.clone());

                        // The Client result

                        socket.write(update.result.as_bytes()).unwrap();
                        socket.write(&[0xA]).unwrap();
                        socket.flush().unwrap();

                        // Debug

                        let inst = update.process.instruction;
                        let mut val = update.process.value;

                        let inst = match inst {
                            Instruction::Get => {
                                val = update.result;
                                "GET"
                            }
                            Instruction::Set => {
                                // Save to DBfile

                                let map = data.lock().unwrap();
                                let file = OpenOptions::new()
                                    .read(true)
                                    .write(true)
                                    .create(true)
                                    .truncate(true)
                                    .open(DB_FILE);

                                let json = serde_json::to_string(&*map).unwrap();
                                file.unwrap().write_all(json.as_bytes()).unwrap();

                                "SET"
                            }
                            Instruction::Nop => "NOP",
                        };

                        println!(
                            "{} | {} | {} {} {}",
                            Utc::now().format(UTC_FORMAT),
                            client.addr,
                            inst,
                            update.process.key,
                            val
                        );
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

    // println!("i[{}] k[{}] v[{}]", inst, key, val); // Debug

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
