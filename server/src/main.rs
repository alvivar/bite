use tungstenite::{accept, Message, WebSocket};

use crossbeam_channel::{unbounded, Receiver, Sender};

use std::{
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

    // const TICK: u64 = 30;
    // thread::spawn(move || loop {
    //     sleep(Duration::new(TICK + 1, 0));

    //     subs_sender_clean.send(subs::Command::Clean(TICK)).unwrap();

    //     heartbeat_sender_clean
    //         .send(heartbeat::Command::Clean(TICK))
    //         .unwrap();
    // });

    // New job on incoming connections
    let thread_count = Arc::new(AtomicUsize::new(0));
    let server = TcpListener::bind("0.0.0.0:1984").unwrap(); // Asumming Docker.

    for stream in server.incoming() {
        // Websocket
        let stream = stream.unwrap();
        let ws_stream = stream.try_clone().unwrap();
        let mut websocket = accept(ws_stream).unwrap();

        let map_sender = map_sender.clone();
        let subs_sender = subs_sender.clone();
        let heartbeat_sender = heartbeat_sender.clone();
        let (conn_sndr, conn_recv) = unbounded::<map::Result>();

        // Hearbeat registry
        let addr = stream.peer_addr().unwrap().to_string();
        let ws_clone = stream.try_clone().unwrap();

        heartbeat_sender
            .send(heartbeat::Command::New(addr.to_owned(), ws_clone))
            .unwrap();

        // Handling the connection
        let thread_count_clone = thread_count.clone();

        thread::spawn(move || {
            let id = thread_count_clone.fetch_add(1, Ordering::Relaxed);

            println!("Client {} connected, thread #{} spawned", addr, id);

            handle_conn(
                &mut websocket,
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

    // @todo Thread waiting for CTRL+C (character?).
    println!("Shutting down");
}

fn handle_conn(
    websocket: &mut WebSocket<TcpStream>,
    map_sender: Sender<map::Command>,
    subs_sender: Sender<subs::Command>,
    heartbeat_sender: Sender<heartbeat::Command>,
    conn_sndr: Sender<map::Result>,
    conn_recv: Receiver<map::Result>,
) {
    loop {
        // Get the message
        let addr = websocket.get_ref().peer_addr().unwrap().to_string();

        let message = websocket.read_message().unwrap();
        let content = message.to_text().unwrap();

        // Parse the message
        let proc = parse::proc_from_string(content);
        let instr = proc.instr;
        let key = proc.key;
        let val = proc.value;

        let conn_sender = conn_sndr.clone();

        let async_instr = match instr {
            Instr::Nop => AsyncInstr::No("NOP".to_owned()),

            Instr::Get => {
                if key.len() < 1 {
                    AsyncInstr::No("KEY?".to_owned())
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
                if key.len() < 1 {
                    AsyncInstr::No("KEY?".to_owned())
                } else {
                    map_sender
                        .send(map::Command::Set(key.to_owned(), val.to_owned()))
                        .unwrap();

                    subs_sender.send(subs::Command::Call(key, val)).unwrap();

                    AsyncInstr::No("OK".to_owned())
                }
            }

            Instr::SetIfNone => {
                if key.len() < 1 {
                    AsyncInstr::No("KEY?".to_owned())
                } else {
                    map_sender
                        .send(map::Command::SetIfNone(
                            key.to_owned(),
                            val.to_owned(),
                            subs_sender.clone(),
                        ))
                        .unwrap();

                    // ^ Subscription resolves after map operation.

                    AsyncInstr::No("OK".to_owned())
                }
            }

            Instr::Inc => {
                if key.len() < 1 {
                    AsyncInstr::No("KEY?".to_owned())
                } else {
                    map_sender
                        .send(map::Command::Inc(key, conn_sender, subs_sender.clone()))
                        .unwrap();

                    // ^ Subscription resolves after map operation.

                    AsyncInstr::Yes
                }
            }

            Instr::Append => {
                if key.len() < 1 {
                    AsyncInstr::No("KEY?".to_owned())
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
                if key.len() < 1 {
                    AsyncInstr::No("KEY?".to_owned())
                } else {
                    map_sender.send(map::Command::Delete(key)).unwrap();

                    AsyncInstr::No("OK".to_owned())
                }
            }

            Instr::Signal => {
                if key.len() < 1 {
                    AsyncInstr::No("KEY?".to_owned())
                } else {
                    subs_sender.send(subs::Command::Call(key, val)).unwrap();

                    AsyncInstr::No("OK".to_owned())
                }
            }

            Instr::SubJ | Instr::SubGet | Instr::SubBite => {
                let stream = websocket.get_ref().try_clone().unwrap();
                let mut ws = accept(websocket.get_ref()).unwrap();
                let (sub_sender, sub_receiver) = unbounded::<subs::Result>();

                subs_sender
                    .send(subs::Command::New(sub_sender, key, instr))
                    .unwrap();

                loop {
                    let message = match sub_receiver.recv().unwrap() {
                        subs::Result::Message(msg) => msg,

                        subs::Result::Ping => {
                            if let Err(e) = heartbeat::beat(&stream) {
                                println!("Client {} subscription ping error: {}", addr, e);
                                break;
                            }

                            continue;
                        }
                    };

                    if let Err(e) = ws.write_message(Message::Text(message)) {
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

        let mut message = match async_instr {
            AsyncInstr::Yes => match conn_recv.recv().unwrap() {
                map::Result::Message(msg) => {
                    println!("Received! {}", msg);

                    msg
                }
            },

            AsyncInstr::No(msg) => msg,
        };

        // Response
        if message.len() < 1 {
            message = "\0".to_owned();
        }

        websocket.write_message(Message::Text(message)).unwrap();

        heartbeat_sender
            .send(heartbeat::Command::Touch(addr))
            .unwrap();
    }
}

// fn stream_write(mut stream: &TcpStream, message: &str) -> std::io::Result<()> {
//     stream.write(message.as_bytes())?;
//     stream.write(&[0xA])?; // New line
//     stream.flush()?;

//     Ok(())
// }
