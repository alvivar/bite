mod db;
mod map;
mod parse;
mod work;

use std::{
    io::{BufRead, BufReader, BufWriter, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, Receiver, Sender},
};

fn main() {
    let listener = TcpListener::bind("127.0.0.1:1984").unwrap();
    let mut pool = work::ThreadPool::new(4);

    // Map & DB Thread.
    let map = map::Map::new();
    let map_sender = map.sender.clone();

    let mut db = db::DB::new(map.data.clone(), 4);
    let db_sender = db.sender.clone();
    db_sender.send(db::Command::Load).unwrap();

    pool.execute(move || map.handle(db_sender.clone()));
    pool.execute(move || db.handle());

    // New job on incoming connections.
    for stream in listener.incoming() {
        let stream = stream.unwrap();
        let map = map_sender.clone();
        let (sender, receiver) = mpsc::channel::<map::Result>();

        pool.execute(move || {
            handle_connection(stream, map, sender, receiver);
        });
    }

    // @todo Thread waiting for q! in the input to quit.
    println!("Shutting down.");
}

fn handle_connection(
    stream: TcpStream,
    map: Sender<map::Command>,
    sender: Sender<map::Result>,
    receiver: Receiver<map::Result>,
) {
    let mut writer = BufWriter::new(stream.try_clone().unwrap());
    let mut reader = BufReader::new(stream);

    loop {
        let mut buffer = String::new();
        reader.read_line(&mut buffer).unwrap();

        match buffer.len() > 0 {
            true => println!("> {}", buffer.trim()),
            false => {
                println!("Client disconnected.");
                break;
            }
        }

        // Parse the message.
        let proc = parse::from_string(buffer.as_str());
        let instr = proc.instr;
        let key = proc.key;
        let val = proc.value;

        let sender = sender.clone();

        match instr {
            parse::Instr::Get => map.send(map::Command::Get(sender, key)).unwrap(),
            parse::Instr::Set => map.send(map::Command::Set(sender, key, val)).unwrap(),
            parse::Instr::Nop => {
                writer.write("NOP".as_bytes()).unwrap();
                writer.write(&[0xA]).unwrap(); // Write line.
                writer.flush().unwrap();
                continue;
            }
        }

        // Wait for the @map response.
        let response = receiver.recv().unwrap();

        match response {
            map::Result::Message(m) => {
                writer.write(m.as_bytes()).unwrap();
                writer.write(&[0xA]).unwrap(); // Write line.
                writer.flush().unwrap();
            }
        }
    }
}
