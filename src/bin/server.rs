use chrono::Utc;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

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
                    msg_sndr.send(Message::Resp("GET".to_owned())).unwrap();
                }
                Command::Set(key, val) => {
                    msg_sndr.send(Message::Resp("SET".to_owned())).unwrap();
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
    NOP,
}

fn main() -> std::io::Result<()> {
    let key_values: BTreeMap<String, String> = BTreeMap::new();
    let data = Arc::new(Mutex::new(key_values));

    // @todo Thread pool

    let mut workers = Vec::<Worker>::new();

    let (msg_sndr, msg_recv) = mpsc::channel();
    workers.push(Worker::new(msg_sndr.clone()));
    workers.push(Worker::new(msg_sndr.clone()));
    workers.push(Worker::new(msg_sndr.clone()));
    workers.push(Worker::new(msg_sndr.clone()));

    let server = TcpListener::bind("127.0.0.1:1984")?;
    server.set_nonblocking(true)?;

    loop {
        if let Ok((mut socket, addr)) = server.accept() {
            sleep();
        }
    }

    // for

    // for stream in server.incoming() {
    //     match stream {
    //         Ok(mut stream) => {
    //             let key_values = Arc::clone(&data);
    //             let process = parse_message(&mut stream)?;

    //             match (process.instruction, process.key, process.value) {
    //                 (Instruction::GET, k, _) => worker.command(Command::Get(k)),
    //                 (Instruction::SET, k, v) => worker.command(Command::Set(k, v)),
    //                 (Instruction::NOP, _, _) => {
    //                     // @todo Handle nop
    //                 }
    //             }
    //         }
    //         Err(e) => {
    //             println!("Somehow a listener.incoming() error?\n{}\n", e);
    //         }
    //     }
    // }

    Ok(())
}

fn sleep() {
    thread::sleep(::std::time::Duration::from_millis(100));
}

// fn save_map_to_disc(data: &Arc<Mutex<BTreeMap<String, String>>>) {
//     let map = data.lock().unwrap();
//     println!("btree: {:?}", serde_json::to_string(&*map).unwrap());
// }

// fn update_map_worker_thread(proc: Process, data: Arc<Mutex<BTreeMap<String, String>>>) -> String {
//     let mut map = data.lock().unwrap();
//     match proc.instruction {
//         Instruction::GET => match map.get(&proc.key) {
//             Some(content) => {
//                 println!(
//                     "{} GET {} {}",
//                     Utc::now().format("%Y-%m-%d %H:%M:%S"),
//                     proc.key,
//                     content
//                 );

//                 content.clone()
//             }
//             None => {
//                 println!(
//                     "{} GET {}",
//                     Utc::now().format("%Y-%m-%d %H:%M:%S"),
//                     proc.key
//                 );

//                 "".to_owned()
//             }
//         },
//         Instruction::SET => {
//             map.insert(proc.key.to_owned(), proc.value.to_owned());

//             println!(
//                 "{} SET {} {}",
//                 Utc::now().format("%Y-%m-%d %H:%M:%S"),
//                 proc.key,
//                 proc.value.to_owned()
//             );

//             "OK".to_owned()
//         }
//         Instruction::NOP => {
//             println!(
//                 "{} SET {} {}",
//                 Utc::now().format("%Y-%m-%d %H:%M:%S"),
//                 proc.key,
//                 proc.value.to_owned()
//             );

//             "".to_owned()
//         }
//     }
// }

// @old Before threads.
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

// @old
// fn get_process(inst: (String, String, String)) -> Result<Process, String> {
//     match (inst.0.as_str(), inst.1.as_str(), inst.2.as_str()) {
//         ("set", k, v) => Ok(Process {
//             instruction: Instruction::SET,
//             key: k.to_owned(),
//             value: v.to_owned(),
//         }),
//         ("get", k, v) => Ok(Process {
//             instruction: Instruction::GET,
//             key: k.to_owned(),
//             value: v.to_owned(),
//         }),
//         (i, k, v) => Err(format!(
//             "{} NOP {} {} {}",
//             Utc::now().format("%Y-%m-%d %H:%M:%S"),
//             i.to_owned(),
//             k.to_owned(),
//             v.to_owned()
//         )),
//     }
// }

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
