use crossbeam_channel::{unbounded, Receiver, Sender};

use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread::{self, sleep},
    time::Duration,
};

mod db;
mod heartbeat;
mod map;
mod parse;
mod subs;

use db::DB;
use parse::{AsyncInstr, Instr};

fn main() {
    println!("\nBIT:E");

    let listener = TcpListener::bind("0.0.0.0:1984").unwrap(); // Asumming Docker.

    // Map
    let map = map::Map::new();
    let map_sender = map.sender.clone();

    // DB
    let mut db = DB::new(map.data.clone());
    db.load_from_file();

    // Subscriptions
    let subs = subs::Subs::new();
    let subs_sender = subs.sender.clone();
    let subs_sender_clean = subs_sender.clone();

    // Channels
    let db_modified = db.modified.clone();

    thread::spawn(move || map.handle(db_modified));
    thread::spawn(move || db.handle(3));
    thread::spawn(move || subs.handle());

    // Cleaning lost connections & subscriptions
    let heartbeat = heartbeat::Heartbeat::new();
    let heartbeat_sender = heartbeat.sender.clone();
    let heartbeat_sender_clean = heartbeat.sender.clone();

    thread::spawn(move || heartbeat.handle());

    // const TICK: u64 = 5;
    // thread::spawn(move || loop {
    //     sleep(Duration::new(TICK + 1, 0));

    //     subs_sender_clean.send(subs::Command::Clean(TICK)).unwrap();

    //     heartbeat_sender_clean
    //         .send(heartbeat::Command::Clean(TICK))
    //         .unwrap();
    // });

    // New job on incoming connections
    let thread_count = Arc::new(AtomicUsize::new(0));

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        let map_sender = map_sender.clone();
        let subs_sender = subs_sender.clone();
        let heartbeat_sender = heartbeat_sender.clone();
        let (conn_sndr, conn_recv) = unbounded::<map::Result>();

        // Hearbeat register
        let addr = stream.peer_addr().unwrap().to_string();
        let stream_clone = stream.try_clone().unwrap();

        heartbeat_sender
            .send(heartbeat::Command::New(addr.to_owned(), stream_clone))
            .unwrap();

        // New thread handling a connection.
        let thread_count_clone = thread_count.clone();
        thread::spawn(move || {
            let id = thread_count_clone.fetch_add(1, Ordering::Relaxed);

            println!("Client {} connected, thread #{} spawned", addr, id);

            handle_conn(
                stream,
                map_sender,
                subs_sender,
                heartbeat_sender,
                conn_sndr,
                conn_recv,
            );

            println!("Thread #{} terminated", id);

            thread_count_clone.fetch_sub(1, Ordering::Relaxed);
        });
    }

    // @todo Thread waiting q! in the input to quit.
    println!("Shutting down");
}

fn handle_conn(
    stream: TcpStream,
    map_sender: Sender<map::Command>,
    subs_sender: Sender<subs::Command>,
    heartbeat_sender: Sender<heartbeat::Command>,
    conn_sndr: Sender<map::Result>,
    conn_recv: Receiver<map::Result>,
) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());

    loop {
        let addr = stream.peer_addr().unwrap().to_string();

        let mut buffer = String::new();

        if let Err(e) = reader.read_line(&mut buffer) {
            println!("Client {} disconnected: {}", addr, e);
            break;
        }

        if buffer.len() > 0 {
            println!("> {}", buffer.trim());
        } else {
            println!("Client {} disconnected: 0 bytes read", addr);
            break;
        }

        // Parse the message.
        let proc = parse::proc_from_string(buffer.as_str());
        let instr = proc.instr;
        let key = proc.key;
        let val = proc.value;

        let conn_sender = conn_sndr.clone();

        let async_instr = match instr {
            Instr::Nop => AsyncInstr::No("NOP".to_owned()),

            Instr::Get => {
                if key.len() <= 0 {
                    AsyncInstr::No("OK".to_owned())
                } else {
                    map_sender
                        .send(map::Command::Get(key, conn_sender))
                        .unwrap();

                    AsyncInstr::Yes
                }
            }

            Instr::Bite => {
                map_sender
                    .send(map::Command::Bite(key, conn_sender))
                    .unwrap();

                AsyncInstr::Yes
            }

            Instr::Jtrim => {
                map_sender
                    .send(map::Command::Jtrim(key, conn_sender))
                    .unwrap();

                AsyncInstr::Yes
            }

            Instr::Json => {
                map_sender
                    .send(map::Command::Json(key, conn_sender))
                    .unwrap();

                AsyncInstr::Yes
            }

            Instr::Set => {
                if key.len() > 0 {
                    map_sender
                        .send(map::Command::Set(key.to_owned(), val.to_owned()))
                        .unwrap();

                    subs_sender.send(subs::Command::Call(key, val)).unwrap();
                }

                AsyncInstr::No("OK".to_owned())
            }

            Instr::SetIfNone => {
                if key.len() > 0 {
                    map_sender
                        .send(map::Command::SetIfNone(
                            key.to_owned(),
                            val.to_owned(),
                            subs_sender.clone(),
                        ))
                        .unwrap();

                    // ^ Subscription resolves after the map operation.
                }

                AsyncInstr::No("OK".to_owned())
            }

            Instr::Inc => {
                if key.len() <= 0 {
                    AsyncInstr::No("NOP".to_owned())
                } else {
                    map_sender
                        .send(map::Command::Inc(key, conn_sender, subs_sender.clone()))
                        .unwrap();

                    // ^ Subscription resolves after the map operation.

                    AsyncInstr::Yes
                }
            }

            Instr::Append => {
                if key.len() <= 0 {
                    AsyncInstr::No("NOP".to_owned())
                } else {
                    map_sender
                        .send(map::Command::Append(
                            key,
                            val,
                            conn_sender,
                            subs_sender.clone(),
                        ))
                        .unwrap();

                    // ^ Subscription resolves after the map operation.

                    AsyncInstr::Yes
                }
            }

            Instr::Delete => {
                if key.len() <= 0 {
                    AsyncInstr::No("KEY?".to_owned())
                } else {
                    map_sender.send(map::Command::Delete(key)).unwrap();

                    AsyncInstr::No("OK".to_owned())
                }
            }

            Instr::Signal => {
                if key.len() > 0 {
                    subs_sender.send(subs::Command::Call(key, val)).unwrap();
                }

                AsyncInstr::No("OK".to_owned())
            }

            Instr::SubJ | Instr::SubGet | Instr::SubBite => {
                let stream = stream.try_clone().unwrap();
                let (sub_sender, sub_receiver) = unbounded::<subs::Result>();

                subs_sender
                    .send(subs::Command::New(sub_sender, key, instr))
                    .unwrap();

                loop {
                    let message = match sub_receiver.recv().unwrap() {
                        subs::Result::Message(msg) => msg,

                        subs::Result::Ping => {
                            if let Err(e) = beat(&stream) {
                                println!("Client {} subscription ping failed: {}", addr, e);
                                break;
                            }

                            continue;
                        }
                    };

                    if let Err(e) = stream_write(&stream, message.as_str()) {
                        println!("Client {} disconnected: {}", addr, e);
                        break;
                    } else {
                        heartbeat_sender
                            .send(heartbeat::Command::Touch(addr.to_owned()))
                            .unwrap();
                    }
                }

                return;
            }
        };

        let message = match async_instr {
            AsyncInstr::Yes => match conn_recv.recv().unwrap() {
                map::Result::Message(msg) => msg,
            },

            AsyncInstr::No(msg) => msg,
        };

        stream_write(&stream, message.as_str()).unwrap();

        heartbeat_sender
            .send(heartbeat::Command::Touch(addr))
            .unwrap();
    }
}

fn stream_write(mut stream: &TcpStream, message: &str) -> std::io::Result<()> {
    stream.write(message.as_bytes())?;
    stream.write(&[0xA])?; // Write line.
    stream.flush()?;

    Ok(())
}

fn beat(mut stream: &TcpStream) -> std::io::Result<()> {
    stream.write(&[0xA])?; // Write line.
    stream.flush()?;

    Ok(())
}
