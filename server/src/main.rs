mod db;
mod map;
mod work;

mod parse;
use parse::{AsyncInstr, Instr};

use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, Receiver, Sender},
};

fn main() {
    println!("\nBITE\n");

    let listener = TcpListener::bind("0.0.0.0:1984").unwrap(); // Asumming Docker.
    let mut pool = work::ThreadPool::new(4);

    // Map & DB Thread.
    let map = map::Map::new();
    let map_sender = map.sender.clone();

    let mut db = db::DB::new(map.data.clone());
    db.load_from_file();

    let db_modified = db.modified.clone();

    pool.execute(move || map.handle(db_modified));
    pool.execute(move || db.handle(3));

    // New job on incoming connections.
    for stream in listener.incoming() {
        let stream = stream.unwrap();
        let map_sender = map_sender.clone();
        let (conn_sender, conn_receiver) = mpsc::channel::<map::Result>();

        pool.execute(move || handle_conn(stream, map_sender, conn_sender, conn_receiver));
    }

    // @todo Thread waiting q! in the input to quit.
    println!("Shutting down.");
}

fn handle_conn(
    mut stream: TcpStream,
    map_sndr: Sender<map::Command>,
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
            Instr::Get => {
                map_sndr.send(map::Command::Get(conn_sndr, key)).unwrap();
                AsyncInstr::Yes
            }
            Instr::Set => {
                map_sndr.send(map::Command::Set(key, val)).unwrap();
                AsyncInstr::No(String::from("OK"))
            }
            Instr::Json => {
                map_sndr.send(map::Command::Json(conn_sndr, key)).unwrap();
                AsyncInstr::Yes
            }
            Instr::Nop => AsyncInstr::No(String::from("NO")),
        };

        let message = match async_instr {
            AsyncInstr::Yes => match conn_recvr.recv().unwrap() {
                map::Result::Message(msg) => msg,
            },
            AsyncInstr::No(msg) => msg,
        };

        stream.write(message.as_bytes()).unwrap();
        stream.write(&[0xA]).unwrap(); // Write line.
        stream.flush().unwrap();
    }
}
