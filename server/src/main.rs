use crossbeam_channel::{unbounded, Receiver, Sender};

use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
};

mod db;
mod map;
mod parse;
mod subs;
mod work;

use db::DB;
use parse::{AsyncInstr, Instr};

fn main() {
    println!("\nBIT:E");

    let listener = TcpListener::bind("0.0.0.0:1984").unwrap(); // Asumming Docker.
    let mut pool = work::ThreadPool::new(8);

    // Map
    let map = map::Map::new();
    let map_sender = map.sender.clone();

    // DB
    let mut db = DB::new(map.data.clone());
    db.load_from_file();

    // Subscritions
    let subs = subs::Subs::new();
    let sub_sender = subs.sender.clone();

    // Channels
    let db_modified = db.modified.clone();
    let map_subsender = sub_sender.clone();

    pool.execute(move || map.handle(db_modified, map_subsender));
    pool.execute(move || db.handle(3));
    pool.execute(move || subs.handle());

    // New job on incoming connections.
    for stream in listener.incoming() {
        let stream = stream.unwrap();
        let map_sender = map_sender.clone();
        let sub_sender = sub_sender.clone();
        let (conn_sender, conn_receiver) = unbounded::<map::Result>();

        pool.execute(move || {
            handle_conn(stream, map_sender, sub_sender, conn_sender, conn_receiver)
        });
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

        let conn_sender = conn_sndr.clone();

        let async_instr = match instr {
            Instr::Nop => AsyncInstr::No("NOP".to_owned()),

            Instr::Get => {
                if key.len() <= 0 {
                    AsyncInstr::No("OK".to_owned())
                } else {
                    map_sender
                        .send(map::Command::Get(conn_sender, key))
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
                    .send(map::Command::Json(conn_sender, key))
                    .unwrap();

                AsyncInstr::Yes
            }

            Instr::Jtrim => {
                map_sender
                    .send(map::Command::Jtrim(conn_sender, key))
                    .unwrap();

                AsyncInstr::Yes
            }

            Instr::SubJ | Instr::SubGet | Instr::SubBite => {
                let stream = stream.try_clone().unwrap();
                let (sub_sender, sub_receiver) = unbounded::<map::Result>();

                subs_sender
                    .send(subs::Command::New(sub_sender, key, instr))
                    .unwrap();

                loop {
                    let message = match sub_receiver.recv().unwrap() {
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
    // @todo There is a bug here, when the first write works and the second
    // fails unwrapping, usually when there are subscriptions a la user, then
    // user.name, then user.name.meaning.

    stream.write(message.as_bytes())?;
    stream.write(&[0xA])?; // Write line.
    stream.flush()?;

    Ok(())
}
