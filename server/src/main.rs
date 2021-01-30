mod parse;
mod work;

use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, BufWriter, Write},
    net::{TcpListener, TcpStream},
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
};

enum Response {
    Message(String),
}

enum Map {
    Get(Sender<Response>, String),
    Set(Sender<Response>, String, String),
}

fn main() {
    let data = Arc::new(Mutex::new(BTreeMap::<String, String>::new()));
    let listener = TcpListener::bind("127.0.0.1:1984").unwrap();
    let pool = work::ThreadPool::new(8);

    // Map Thread.
    let (map_sender, map_receiver) = mpsc::channel::<Map>();
    pool.execute(move || {
        let map = data.clone();

        loop {
            let message = map_receiver.recv().unwrap();
            match message {
                Map::Get(handle, key) => {
                    let map = map.lock().unwrap();
                    match map.get(&key) {
                        Some(value) => handle.send(Response::Message(value.to_owned())).unwrap(),
                        None => handle.send(Response::Message("".to_owned())).unwrap(),
                    }
                }
                Map::Set(handle, key, value) => {
                    let mut map = map.lock().unwrap();
                    match map.insert(key, value) {
                        Some(_) => handle.send(Response::Message("OK".to_owned())).unwrap(),
                        None => handle.send(Response::Message("NOP".to_owned())).unwrap(),
                    }
                }
            }
        }
    });

    // A thread for each incoming connection.
    for stream in listener.incoming() {
        let stream = stream.unwrap();
        let map = map_sender.clone();

        let (sender, receiver) = mpsc::channel::<Response>();
        pool.execute(move || {
            handle_connection(stream, map, sender, receiver);
        });
    }

    println!("Shutting down.");
}

fn handle_connection(
    stream: TcpStream,
    map: Sender<Map>,
    sender: Sender<Response>,
    receiver: Receiver<Response>,
) {
    let mut writer = BufWriter::new(stream.try_clone().unwrap());
    let mut reader = BufReader::new(stream);

    loop {
        let mut buffer = String::new();
        reader.read_line(&mut buffer).unwrap();

        match buffer.len() > 0 {
            true => println!("> {}", buffer.trim()),
            false => {
                println!("Client disconnected");
                break;
            }
        }

        // Parse the message.
        let process = parse::from_message(buffer.as_str());
        let instr = process.instruction;
        let key = process.key;
        let val = process.value;

        let sender = sender.clone();
        match instr {
            parse::Instr::Get => map.send(Map::Get(sender, key)).unwrap(),
            parse::Instr::Set => map.send(Map::Set(sender, key, val)).unwrap(),
            parse::Instr::Nop => {}
        }

        // Wait for the @map response.
        let response = receiver.recv().unwrap();
        match response {
            Response::Message(m) => {
                writer.write(m.as_bytes()).unwrap();
                writer.write(&[0xA]).unwrap(); // Write line.
                writer.flush().unwrap();
            }
        }
    }
}
