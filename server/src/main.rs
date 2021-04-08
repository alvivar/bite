use crossbeam_channel::{unbounded, Receiver, Sender};

use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread::{self, sleep},
    time::Duration,
};

use std::sync::atomic::{AtomicUsize, Ordering};

mod db;
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

    // Channels
    let db_modified = db.modified.clone();

    thread::spawn(move || map.handle(db_modified));
    thread::spawn(move || db.handle(3));
    thread::spawn(move || subs.handle());

    // Connections & Subscritions maintenance heartbeat
    // let streams = Arc::new(Mutex::new(Vec::<TcpStream>::new()));
    // let streams_clone = streams.clone();
    let sub_sender_clean = subs_sender.clone();

    thread::spawn(move || loop {
        sleep(Duration::new(90, 0));

        // Cleaning subscriptions
        sub_sender_clean.send(subs::Command::Clean(90)).unwrap();

        //     // Cleaning clients
        //     let mut streams_lock = streams.lock().unwrap();
        //     let mut orphans = Vec::<usize>::new();

        //     for (i, s) in streams_lock.iter().enumerate() {
        //         if let Err(e) = stream_write(s, "\0") {
        //             orphans.push(i);
        //             println!("Client error on ping: {}", e);
        //         }
        //     }

        //     for &i in orphans.iter().rev() {
        //         streams_lock.swap_remove(i);
        //     }
    });

    // New job on incoming connections
    let thread_count = Arc::new(AtomicUsize::new(0));

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        let map_sender = map_sender.clone();
        let subs_sender = subs_sender.clone();
        let (conn_sndr, conn_recv) = unbounded::<map::Result>();

        // Save the stream to check later for connections and clean up.
        // let stream_clone = stream.try_clone().unwrap();
        // let streams_clone = streams_clone.clone();
        // streams_clone.lock().unwrap().push(stream_clone); // @todo When this lock gets released?

        // New thread handling a connection.
        let thread_count_clone = thread_count.clone();
        thread::spawn(move || {
            let id = thread_count_clone.fetch_add(1, Ordering::Relaxed);

            println!("Client connected, thread #{} spawned", id);
            handle_conn(stream, map_sender, subs_sender, conn_sndr, conn_recv);
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
    conn_sndr: Sender<map::Result>,
    conn_recv: Receiver<map::Result>,
) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());

    loop {
        let mut buffer = String::new();

        if let Err(e) = reader.read_line(&mut buffer) {
            println!("Client disconnected: {}", e);
            break;
        }

        if buffer.len() > 0 {
            println!("> {}", buffer.trim());
        } else {
            println!("Client disconnected: 0 bytes read");
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

            Instr::Json => {
                map_sender
                    .send(map::Command::Json(key, conn_sender))
                    .unwrap();

                AsyncInstr::Yes
            }

            Instr::Jtrim => {
                map_sender
                    .send(map::Command::Jtrim(key, conn_sender))
                    .unwrap();

                AsyncInstr::Yes
            }

            Instr::SubJ | Instr::SubGet | Instr::SubBite => {
                let mut stream = stream.try_clone().unwrap();
                let (sub_sender, sub_receiver) = unbounded::<subs::Result>();

                subs_sender
                    .send(subs::Command::New(sub_sender, key, instr))
                    .unwrap();

                loop {
                    let message = match sub_receiver.recv().unwrap() {
                        subs::Result::Message(msg) => msg,

                        subs::Result::Ping => {
                            if let Err(e) = stream.write(&[0]) {
                                println!("Client disconnected on subscription ping: {}", e);
                                break;
                            }

                            continue;
                        }
                    };

                    if let Err(e) = stream_write(&stream, message.as_str()) {
                        println!("Client disconnected: {}", e);
                        break;
                    }
                }

                return;
            }

            Instr::Inc => {
                if key.len() <= 0 {
                    AsyncInstr::No("NOP".to_owned())
                } else {
                    map_sender
                        .send(map::Command::Inc(key, conn_sender, subs_sender.clone()))
                        .unwrap();

                    AsyncInstr::Yes
                }
            }
        };

        let message = match async_instr {
            AsyncInstr::Yes => match conn_recv.recv().unwrap() {
                map::Result::Message(msg) => msg,
            },

            AsyncInstr::No(msg) => msg,
        };

        stream_write(&stream, message.as_str()).unwrap();
    }
}

fn stream_write(mut stream: &TcpStream, message: &str) -> std::io::Result<()> {
    stream.write(message.as_bytes())?;
    stream.write(&[0xA])?; // Write line.
    stream.flush()?;

    Ok(())
}
