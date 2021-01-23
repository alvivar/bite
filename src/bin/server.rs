use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, RwLock};
use std::{collections::BTreeMap, thread};
use std::{
    fs::write,
    io::{BufRead, BufReader, Write},
    sync::mpsc,
};

#[derive(Debug)]
enum Command {
    Get(String),
    Set(String, String),
}
#[derive(Debug)]
enum Message {
    Resp(String),
}

struct Worker {
    join_handle: thread::JoinHandle<()>,
    cmd_sndr: mpsc::Sender<Command>,
}

impl Worker {
    pub fn new(msg_sndr: mpsc::Sender<Message>) -> Self {
        let (cmd_sndr, cmd_recv) = mpsc::channel::<Command>();
        let join_handle = thread::spawn(move || Self::worker(cmd_recv, msg_sndr));
        Self {
            join_handle,
            cmd_sndr,
        }
    }

    pub fn command(&self, cmd: Command) {
        self.cmd_sndr.send(cmd).unwrap()
    }

    fn worker(cmd_recv: mpsc::Receiver<Command>, msg_sndr: mpsc::Sender<Message>) {
        loop {
            let cmd = cmd_recv.recv().unwrap();
            println!("{:?}", &cmd);
            match cmd {
                Command::Get(key) => {
                    msg_sndr.send(Message::Resp("Got it".to_owned()));
                }
                Command::Set(key, val) => {
                    msg_sndr.send(Message::Resp("Got it".to_owned()));
                }
            }
        }
    }
}

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

    // TODO: Thread pool

    let (msg_sndr, msg_recv) = mpsc::channel();
    let worker = Worker::new(msg_sndr);

    let listener = TcpListener::bind("127.0.0.1:1984")?;
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let process = parse_message(&mut stream).unwrap();

                worker.command();

                // Current: Listening on the same thread as you're processing things

                // Better: Listen on one thread, handle connections on another

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

fn parse_message(stream: &mut TcpStream) -> std::io::Result<Process> {
    let mut reader = BufReader::new(stream);
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

    let instruction = match inst.trim().to_lowercase().as_str() {
        "get" => Instruction::GET,
        "set" => Instruction::SET,
    };

    Ok(Process {
        instruction,
        key: key.trim().to_owned(),
        value: val.trim().to_owned(),
    })
}
