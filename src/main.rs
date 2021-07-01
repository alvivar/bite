use std::{
    collections::HashMap,
    io,
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
};

use crossbeam_channel::unbounded;
use polling::{Event, Poller};

mod conn;
mod db;
mod map;
mod parse;
mod pool;
mod reader;
mod ready;
mod subs;
mod writer;

use conn::Connection;
use db::DB;
use pool::ThreadPool;
use reader::Reader;
use ready::Ready;
use subs::Subs;
use writer::Writer;

pub enum Work {
    Read(Connection),
    WriteKeyVal(Connection, String, String),
    WriteVal(Connection, String),
}

fn main() -> io::Result<()> {
    // The server and the smol Poller.
    let server = TcpListener::bind("0.0.0.0:1984")?;
    server.set_nonblocking(true)?;

    let poller = Poller::new()?;
    poller.add(&server, Event::readable(0))?;
    let poller = Arc::new(poller);

    let read_map = HashMap::<usize, Connection>::new();
    let read_map = Arc::new(Mutex::new(read_map));

    let write_map = HashMap::<usize, Connection>::new();
    let write_map = Arc::new(Mutex::new(write_map));

    // Map, DB & threads
    let map = map::Map::new();
    let map_tx = map.tx.clone();

    let mut db = DB::new(map.data.clone());
    let db_modified = db.modified.clone();
    db.load_from_file();

    thread::spawn(move || map.handle(db_modified));
    thread::spawn(move || db.handle(3));

    // Thread that re-register the connection for more reading events, and to be
    // written again after the thread pool finished.
    let ready_poller = poller.clone();
    let ready_read_map = read_map.clone();
    let ready_write_map = write_map.clone();
    let ready = Ready::new(ready_poller, ready_read_map, ready_write_map);
    let ready_tx = ready.tx.clone();
    thread::spawn(move || ready.handle());

    // Channels to send work to the thread pool that handles reading and
    // writing.
    let (work_tx, work_rx) = unbounded::<Work>();
    let work_rx = Arc::new(Mutex::new(work_rx));

    // Thread that handles subscriptions, sends writing jobs to the thread when
    // needed.
    let subs_write_map = write_map.clone();
    let subs_work_tx = work_tx.clone();
    let mut subs = Subs::new(subs_write_map, subs_work_tx);
    let subs_tx = subs.tx.clone();
    thread::spawn(move || subs.handle());

    // A pool of threads handling reads and writes. Reading also talks with the
    // the subscription system.
    let mut work = ThreadPool::new(4);

    for _ in 0..work.size() {
        let write_map = write_map.clone();
        let map_tx = map_tx.clone();

        let work_tx = work_tx.clone();
        let work_rx = work_rx.clone();
        let subs_tx = subs_tx.clone();
        let ready_tx = ready_tx.clone();

        work.submit(move || loop {
            let reader_write_map = write_map.clone();
            let map_tx = map_tx.clone();
            let work_tx = work_tx.clone();
            let reader_subs_tx = subs_tx.clone();
            let reader_ready_tx = ready_tx.clone();
            let reader = Reader::new(
                reader_write_map,
                map_tx,
                work_tx,
                reader_subs_tx,
                reader_ready_tx,
            );

            let writer_subs_tx = subs_tx.clone();
            let writer_ready_tx = ready_tx.clone();
            let writer = Writer::new(writer_subs_tx, writer_ready_tx);

            let work = work_rx.lock().unwrap().recv().unwrap();
            match work {
                Work::Read(conn) => reader.handle(conn),
                Work::WriteKeyVal(conn, key, value) => writer.handle_kv(conn, key, value),
                Work::WriteVal(conn, value) => writer.handle_v(conn, value),
            }
        });
    }

    // Connections and events via smol Poller.
    let mut id: usize = 1;
    let mut events = Vec::new();

    loop {
        events.clear();
        poller.wait(&mut events, None)?;

        for ev in &events {
            match ev.key {
                0 => {
                    let (read_socket, addr) = server.accept()?;
                    read_socket.set_nonblocking(true)?;
                    let work_socket = read_socket.try_clone().unwrap();

                    println!("Connection #{} from {}", id, addr);

                    // Register the reading socket for reading events, and save it.
                    poller.add(&read_socket, Event::readable(id))?;

                    read_map
                        .lock()
                        .unwrap()
                        .insert(id, Connection::new(id, read_socket, addr));

                    // Save the writing socket for later.
                    write_map
                        .lock()
                        .unwrap()
                        .insert(id, Connection::new(id, work_socket, addr));

                    // One more connection.
                    id += 1;

                    // The server continues listening for more clients, always 0.
                    poller.modify(&server, Event::readable(0))?;
                }

                id => {
                    if let Some(conn) = read_map.lock().unwrap().remove(&id) {
                        work_tx.send(Work::Read(conn)).unwrap();
                    }
                }
            }
        }
    }
}