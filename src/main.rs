mod conn;
mod data;
mod db;
mod parse;
mod reader;
mod subs;
mod writer;

use crate::conn::Connection;
use crate::data::Data;
use crate::db::DB;
use crate::reader::{Cmd::Read, Reader};
use crate::subs::Subs;
use crate::writer::Writer;

use polling::{Event, Poller};

use std::collections::HashMap;
use std::io;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

fn main() -> io::Result<()> {
    println!("\nBIT:E\n");

    // The server and the smol Poller.
    let server = TcpListener::bind("0.0.0.0:1984")?;
    server.set_nonblocking(true)?;

    let poller = Poller::new()?;
    poller.add(&server, Event::readable(0))?;
    let poller = Arc::new(poller);

    // The connections
    let readers = HashMap::<usize, Connection>::new();
    let readers = Arc::new(Mutex::new(readers));
    let writers = HashMap::<usize, Connection>::new();
    let writers = Arc::new(Mutex::new(writers));

    // The reader
    let reader = Reader::new(poller.clone(), readers.clone(), writers.clone());
    let reader_tx = reader.tx.clone();

    // The writer
    let writer = Writer::new(poller.clone(), readers.clone(), writers.clone());
    let subs_writer_tx = writer.tx.clone();
    let data_writer_tx = writer.tx.clone();
    let reader_writer_tx = writer.tx.clone();

    // Subs
    let mut subs = Subs::new(subs_writer_tx);
    let writer_subs_tx = subs.tx.clone();
    let data_subs_tx = subs.tx.clone();
    let reader_subs_tx = subs.tx.clone();

    // Data & DB
    let data = Data::new(data_writer_tx, data_subs_tx);
    let data_map = data.map.clone();
    let data_tx = data.tx.clone();

    let mut db = DB::new(data_map);
    let db_modified = db.modified.clone();
    db.load_from_file();

    // Threads
    thread::spawn(move || db.handle(3));
    thread::spawn(move || data.handle(db_modified));
    thread::spawn(move || subs.handle());
    thread::spawn(move || writer.handle(writer_subs_tx));
    thread::spawn(move || reader.handle(data_tx, reader_writer_tx, reader_subs_tx));

    // Connections and events via smol Poller.
    let mut id: usize = 1; // 0 belongs to the main TcpListener.
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

                    println!("Connection #{} from {}", id, addr);

                    // The server continues listening for more clients, always 0.
                    poller.modify(&server, Event::readable(0))?;

                    // Register the reader socket for reading events.
                    poller.add(&reader, Event::readable(id))?;
                    readers
                        .lock()
                        .unwrap()
                        .insert(id, Connection::new(id, reader, addr));

                    // Save the writer socket for later use.
                    writers
                        .lock()
                        .unwrap()
                        .insert(id, Connection::new(id, writer, addr));

                    // One more.
                    id += 1;
                }

                id if ev.readable => reader_tx.send(Read(id)).unwrap(),

                _ => unreachable!(),
            }
        }
    }
}
