mod cleaner;
mod connection;
mod data;
mod db;
mod heartbeat;
mod message;
mod parser;
mod reader;
mod subs;
mod writer;

use std::{
    collections::{HashMap, VecDeque},
    env, io,
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
};

use crate::{
    cleaner::Cleaner,
    connection::Connection,
    data::Data,
    db::DB,
    heartbeat::Heartbeat,
    parser::Parser,
    reader::{Action::Read, Reader},
    subs::Subs,
    writer::{
        Action::{Queue, Write},
        Order, Writer,
    },
};

use polling::{Event, Events, Poller};

#[macro_use]
extern crate log;
extern crate pretty_env_logger;

fn main() -> io::Result<()> {
    pretty_env_logger::init();

    info!("BIT:E");

    // Address by config if needed.
    let server = match env::var("SERVER") {
        Ok(var) => var,
        Err(_) => "0.0.0.0:1984".into(),
    };

    info!("Running at {server} | To change the address use the SERVER environment variable");

    // The server and the smol Poller.
    let server = TcpListener::bind(server)?;
    server.set_nonblocking(true)?;

    let poller = Poller::new()?;
    unsafe {
        poller.add(&server, Event::readable(0))?; // 0 is the server.
    }
    let poller = Arc::new(poller);

    // The connections
    let readers = HashMap::<usize, Connection>::new();
    let readers = Arc::new(Mutex::new(readers));
    let writers = HashMap::<usize, Connection>::new();
    let writers = Arc::new(Mutex::new(writers));
    let used_ids = Arc::new(Mutex::new(VecDeque::<usize>::new()));

    // The reader
    let mut reader = Reader::new(poller.clone(), readers.clone());
    let reader_tx = reader.tx.clone();

    // The writer
    let writer = Writer::new(poller.clone(), writers.clone());
    let writer_tx = writer.tx.clone();
    let subs_writer_tx = writer.tx.clone();
    let data_writer_tx = writer.tx.clone();
    let parser_writer_tx = writer.tx.clone();
    let heartbeat_writer_tx = writer.tx.clone();

    // The parser
    let parser = Parser::new();
    let reader_parser_tx = parser.tx.clone();

    // Subs
    let mut subs = Subs::new();
    let data_subs_tx = subs.tx.clone();
    let parser_subs_tx = subs.tx.clone();
    let cleaner_subs_tx = subs.tx.clone();

    // Data & DB
    let data = Data::new(data_writer_tx, data_subs_tx);
    let data_map = data.map.clone();
    let parser_data_tx = data.tx.clone();

    let mut db = DB::new(data_map);
    let db_modified = db.modified.clone();
    db.load_from_file();

    // Cleaner
    let cleaner = Cleaner::new(
        poller.clone(),
        readers.clone(),
        writers.clone(),
        used_ids.clone(),
    );
    let reader_cleaner_tx = cleaner.tx.clone();
    let writer_cleaner_tx = cleaner.tx.clone();

    // Heartbeat
    let heartbeat = Heartbeat::new(readers.clone(), writers.clone());

    // Threads
    thread::spawn(move || reader.handle(reader_parser_tx, reader_cleaner_tx));
    thread::spawn(move || writer.handle(writer_cleaner_tx));
    thread::spawn(move || parser.handle(parser_data_tx, parser_writer_tx, parser_subs_tx));
    thread::spawn(move || subs.handle(subs_writer_tx));
    thread::spawn(move || data.handle(db_modified));
    thread::spawn(move || db.handle(4));
    thread::spawn(move || cleaner.handle(cleaner_subs_tx));
    thread::spawn(move || heartbeat.handle(heartbeat_writer_tx));

    // Connections and events via smol Poller.
    let mut id_count: usize = 1; // 0 belongs to the main TcpListener.
    let mut events = Events::new();

    loop {
        events.clear();
        poller.wait(&mut events, None)?;

        for ev in events.iter() {
            match ev.key {
                0 => {
                    let (reader, addr) = server.accept()?;
                    reader.set_nonblocking(true)?;
                    let writer = reader.try_clone().unwrap();

                    // Reusing ids.
                    let used_id = used_ids.lock().unwrap().pop_front();
                    let client_id = if let Some(id) = used_id {
                        id
                    } else {
                        let id = id_count;
                        id_count += 1;
                        id
                    };

                    info!("Connection #{client_id} from {addr}");

                    // The server continues listening for more clients, always 0.
                    poller.modify(&server, Event::readable(0))?;

                    // Register the reader socket for reading events.
                    unsafe {
                        poller.add(&reader, Event::readable(client_id))?;
                    }
                    readers
                        .lock()
                        .unwrap()
                        .insert(client_id, Connection::new(client_id, reader, addr));

                    // Save the writer socket for later use.
                    unsafe {
                        poller.add(&writer, Event::none(client_id))?;
                    }
                    writers
                        .lock()
                        .unwrap()
                        .insert(client_id, Connection::new(client_id, writer, addr));

                    // The first message to the client is his id, so it can add
                    // it on all his messages or it would get disconnected.
                    writer_tx
                        .send(Queue(Order {
                            from_id: client_id,
                            to_id: client_id,
                            msg_id: 0,
                            data: [].into(),
                        }))
                        .unwrap();
                }

                id if ev.readable => reader_tx.send(Read(id)).unwrap(),

                id if ev.writable => writer_tx.send(Write(id)).unwrap(),

                _ => unreachable!(),
            }
        }
    }
}
