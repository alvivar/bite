mod connection;
mod data;
mod db;
mod message;
mod parser;
mod reader;
mod subs;
mod writer;

use crate::connection::Connection;
use crate::data::Data;
use crate::db::DB;
use crate::parser::Parser;
use crate::reader::{Action::Read, Reader};
use crate::subs::Subs;
use crate::writer::Action::Queue;
use crate::writer::Order;
use crate::writer::{Action::Write, Writer};

use polling::{Event, Poller};

use std::collections::HashMap;
use std::io;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

fn main() -> io::Result<()> {
    println!("\nBIT:E");

    // The server and the smol Poller.
    let server = TcpListener::bind("0.0.0.0:1984")?;
    server.set_nonblocking(true)?;

    let poller = Poller::new()?;
    poller.add(&server, Event::readable(0))?; // 0 is the server.
    let poller = Arc::new(poller);

    // The connections
    let readers = HashMap::<usize, Connection>::new();
    let readers = Arc::new(Mutex::new(readers));
    let writers = HashMap::<usize, Connection>::new();
    let writers = Arc::new(Mutex::new(writers));

    // The reader
    let mut reader = Reader::new(poller.clone(), readers.clone(), writers.clone());
    let reader_tx = reader.tx.clone();

    // The writer
    let writer = Writer::new(poller.clone(), readers.clone(), writers.clone());
    let writer_tx = writer.tx.clone();
    let subs_writer_tx = writer.tx.clone();
    let data_writer_tx = writer.tx.clone();
    let parser_writer_tx = writer.tx.clone();

    // The parser
    let parser = Parser::new();
    let reader_parser_tx = parser.tx.clone();

    // Subs
    let mut subs = Subs::new();
    let writer_subs_tx = subs.tx.clone();
    let data_subs_tx = subs.tx.clone();
    let reader_subs_tx = subs.tx.clone();
    let parser_subs_tx = subs.tx.clone();

    // Data & DB
    let data = Data::new(data_writer_tx, data_subs_tx);
    let data_map = data.map.clone();
    let parser_data_tx = data.tx.clone();

    let mut db = DB::new(data_map);
    let db_modified = db.modified.clone();
    db.load_from_file();

    // Threads
    thread::spawn(move || db.handle(3));
    thread::spawn(move || data.handle(db_modified));
    thread::spawn(move || subs.handle(subs_writer_tx));
    thread::spawn(move || writer.handle(writer_subs_tx));
    thread::spawn(move || reader.handle(reader_parser_tx, reader_subs_tx));
    thread::spawn(move || parser.handle(parser_data_tx, parser_writer_tx, parser_subs_tx));

    // Connections and events via smol Poller.
    let mut id_count: usize = 1; // 0 belongs to the main TcpListener.
    let mut events = Vec::new();

    loop {
        events.clear();
        poller.wait(&mut events, None)?;

        for ev in &events {
            match ev.key {
                0 => {
                    let (reader, addr) = server.accept()?;
                    reader.set_nonblocking(true)?;
                    let writer = reader.try_clone().unwrap();

                    println!("\nConnection #{} from {}", id_count, addr);

                    // The server continues listening for more clients, always 0.
                    poller.modify(&server, Event::readable(0))?;

                    // Register the reader socket for reading events.
                    poller.add(&reader, Event::readable(id_count))?;
                    readers
                        .lock()
                        .unwrap()
                        .insert(id_count, Connection::new(id_count, reader, addr));

                    // Save the writer socket for later use.
                    poller.add(&writer, Event::none(id_count))?;
                    writers
                        .lock()
                        .unwrap()
                        .insert(id_count, Connection::new(id_count, writer, addr));

                    // The first message to the client is his id, so it can add
                    // it on all his messages or it would get disconnected.
                    writer_tx
                        .send(Queue(Order {
                            from_id: id_count,
                            msg_id: 0,
                            data: [].into(),
                        }))
                        .unwrap();

                    // Next.
                    id_count += 1;
                }

                id if ev.readable => reader_tx.send(Read(id)).unwrap(),

                id if ev.writable => writer_tx.send(Write(id)).unwrap(),

                _ => unreachable!(),
            }
        }
    }
}
