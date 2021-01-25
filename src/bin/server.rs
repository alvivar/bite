use chrono::Utc;
use mpsc::Sender;
use std::io::{self, BufRead, BufReader, Write};
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

struct WorkerPool {
    pool: Mutex<Vec<Worker>>,
}

impl WorkerPool {
    pub fn new() -> WorkerPool {
        WorkerPool {
            pool: Mutex::new(Vec::new()),
        }
    }

    pub fn spawn(&self, worker: Worker) {
        self.pool.lock().unwrap().push(worker);
    }
}

struct Worker {
    handle: thread::JoinHandle<()>,
    sender: mpsc::Sender<Command>,
}

impl Worker {
    pub fn new(results: mpsc::Sender<Message>) -> Self {
        let (sender, receiver) = mpsc::channel::<Command>();
        let join_handle = thread::spawn(move || Self::worker(receiver, results));
        Self {
            handle: join_handle,
            sender,
        }
    }

    pub fn command(&self, cmd: Command) {
        self.sender.send(cmd).expect("Failed sending the command.");
    }

    fn worker(commands: mpsc::Receiver<Command>, results: mpsc::Sender<Message>) {
        let mut clients = Vec::<Client>::new();

        loop {
            // Commands

            if let Ok(command) = commands.try_recv() {
                match command {
                    Command::New(socket, addr) => {
                        println!("{} | {} | Connected", Utc::now().format(UTC_FORMAT), addr);
                        clients.push(Client { socket, addr });
                    }
                }
            }

            // Parse the messages from the clients

            for i in 0..clients.len() {
                let mut socket = clients[i]
                    .socket
                    .try_clone()
                    .expect("Failed cloning the nonblocking socket.");
                socket
                    .set_nonblocking(true)
                    .expect("Fail setting the nonblocking socket.");
                let addr = clients[i].addr;

                let mut reader = BufReader::new(&mut socket);
                let mut content = String::new();
                match reader.read_line(&mut content) {
                    Ok(_) => {
                        // @ Socket closed
                        if content.len() <= 0 {
                            clients.remove(i);
                            println!(
                                "{} | {} | Disconnected",
                                Utc::now().format(UTC_FORMAT),
                                addr
                            );
                            continue;
                        }

                        let process =
                            parse_message(&mut content).expect("Failed parsing the message.");

                        results
                            .send(Message::Result(
                                Client {
                                    socket: socket
                                        .try_clone()
                                        .expect("Failed cloning when sending the result."),
                                    addr,
                                },
                                process,
                            ))
                            .expect("Failed sending the result.");
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

const UTC_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

fn main() {
    let data = Arc::new(Mutex::new(BTreeMap::<String, String>::new()));

    // # Incoming connections.

    let server = TcpListener::bind("127.0.0.1:1984").expect("Failed binding the server.");
    server
        .set_nonblocking(true)
        .expect("Failed setting the nonblocking on the server bind.");

    let (sender, receiver) = mpsc::channel::<Message>();

    // let pool = WorkerPool::new();
    // pool.spawn(Worker::new(sender.clone()));
    // pool.spawn(Worker::new(sender.clone()));

    let mut worker_i = 0;
    let workers = [Worker::new(sender.clone()), Worker::new(sender.clone())];

    // # New worker on incoming connections.

    thread::spawn(move || loop {
        if let Ok((socket, address)) = server.accept() {
            // @ How to ask for the worker handling less connections?
            workers[worker_i].command(Command::New(socket, address));
            worker_i = (worker_i + 1) % workers.len();
        }
    });

    // # The main thread process the map based on responses from the Thread pool.

    loop {
        match receiver.recv() {
            Ok(message) => {
                match message {
                    Message::Result(client, process) => {
                        let mut socket = client.socket.try_clone().expect("");

                        let update = update_btreemap(process, data.clone());

                        socket
                            .write(update.result.as_bytes())
                            .expect("Failed writting in the socket.");
                        socket
                            .write(&[0xA])
                            .expect("Failed writting in the socket.");
                        socket.flush().expect("Failed flusing in the socket.");

                        let ip = client.addr; //address;
                        let now = Utc::now().format(UTC_FORMAT);
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
    let mut map = data.lock().expect("Failed locking the mutex.");
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
