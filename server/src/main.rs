mod map;
mod subs;
mod work;

mod db;
use db::DB;

mod parse;
use parse::{AsyncInstr, Instr};

use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, Receiver, Sender},
    vec,
};

fn main() {
    println!("\nBIT:E");

    let listener = TcpListener::bind("0.0.0.0:1984").unwrap(); // Asumming Docker.
    let mut pool = work::ThreadPool::new(4);

    // Map
    let map = map::Map::new();
    let map_sender = map.sender.clone();

    // DB
    let mut db = DB::new(map.data.clone());
    db.load_from_file();

    // Subscritions
    let subs = subs::Subs::new();
    let subs_sender = subs.sender.clone();

    // Channels
    let db_modified = db.modified.clone();
    let map_subs_sender = subs_sender.clone();
    let subs_map_sender = map_sender.clone();

    pool.execute(move || map.handle(db_modified, map_subs_sender));
    pool.execute(move || db.handle(3));
    pool.execute(move || subs.handle(subs_map_sender));

    // New job on incoming connections.
    for stream in listener.incoming() {
        let stream = stream.unwrap();
        let map_sender = map_sender.clone();
        let subs_sender = subs_sender.clone();
        let (conn_sndr, conn_recvr) = mpsc::channel::<map::Result>();

        pool.execute(move || handle_conn(stream, map_sender, subs_sender, conn_sndr, conn_recvr));
    }

    // @todo Thread waiting q! in the input to quit.
    println!("Shutting down.");
}

fn handle_conn(
    stream: TcpStream,
    map_sender: Sender<map::Command>,
    subs_sender: Sender<subs::Command>,
    conn_sndr: Sender<map::Result>,
    conn_recvr: Receiver<map::Result>,
) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());

    loop {
        let mut buffer = String::new();

        if let Err(e) = reader.read_line(&mut buffer) {
            println!("Client disconnected: {}.", e);
            break;
        }

        if buffer.len() > 0 {
            println!("> {}", buffer.trim());
        } else {
            println!("Client disconnected.");
            break;
        }

        // Parse the message.
        let proc = parse::proc_from_string(buffer.as_str());
        let instr = proc.instr;
        let key = proc.key;
        let val = proc.value;

        let conn_sndr = conn_sndr.clone();

        let async_instr = match instr {
            Instr::Nop => AsyncInstr::No("NOP".to_owned()),
            Instr::Get => {
                if key.len() <= 0 {
                    AsyncInstr::No("OK".to_owned())
                } else {
                    map_sender
                        .send(map::Command::Get(vec![conn_sndr], key))
                        .unwrap();

                    AsyncInstr::Yes
                }
            }
            Instr::Set => {
                if key.len() > 0 {
                    map_sender
                        .send(map::Command::Set(key.to_owned(), val.to_owned()))
                        .unwrap();

                    subs_sender.send(subs::Command::CallSubs(key, val)).unwrap();
                }

                AsyncInstr::No(String::from("OK"))
            }
            Instr::Json => {
                map_sender
                    .send(map::Command::Json(vec![conn_sndr], key))
                    .unwrap();

                AsyncInstr::Yes
            }
            Instr::Jtrim => {
                map_sender
                    .send(map::Command::Jtrim(vec![conn_sndr], key))
                    .unwrap();

                AsyncInstr::Yes
            }
            Instr::SubJtrim | Instr::SubJson | Instr::SubGet => {
                let stream = stream.try_clone().unwrap();
                let (sub_sndr, sub_rcvr) = mpsc::channel::<map::Result>();

                subs_sender
                    .send(subs::Command::NewSub(sub_sndr, key, instr))
                    .unwrap();

                loop {
                    let message = match sub_rcvr.recv().unwrap() {
                        map::Result::Message(msg) => msg,
                        map::Result::Ping => continue,
                    };

                    if let Err(e) = stream_write(&stream, message) {
                        println!("Client disconnected: {}", e);
                        break;
                    }
                }

                return;
            }
        };

        let message = match async_instr {
            AsyncInstr::Yes => match conn_recvr.recv().unwrap() {
                map::Result::Message(msg) => msg,
                map::Result::Ping => continue,
            },
            AsyncInstr::No(msg) => msg,
        };

        stream_write(&stream, message).unwrap();
    }
}

fn stream_write(mut stream: &TcpStream, message: String) -> std::io::Result<()> {
    if let Err(e) = stream.write(message.as_bytes()) {
        return Err(e);
    } else {
        stream.write(&[0xA]).unwrap(); // Write line.
        stream.flush().unwrap();
    }

    Ok(())
}
