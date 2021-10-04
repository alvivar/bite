use std::{
    collections::HashMap,
    io::{self, Read, Write},
    net::TcpListener,
    str::from_utf8,
    sync::{Arc, Mutex},
    thread,
};

use polling::{Event, Poller};

mod conn;
mod data;
mod db;
mod msg;
mod subs;
mod writer;

use conn::Connection;
use data::Data;
use db::DB;
use msg::{needs_key, parse, Instr};
use subs::Subs;
use writer::Writer;

const OK: &str = "OK";
const NOP: &str = "NOP";
const KEY: &str = "KEY?";

fn main() -> io::Result<()> {
    println!("\nbit:e\n");

    // The server and the smol Poller.
    let server = TcpListener::bind("0.0.0.0:1984")?;
    server.set_nonblocking(true)?;

    let poller = Poller::new()?;
    poller.add(&server, Event::readable(0))?;
    let poller = Arc::new(poller);

    let mut readers = HashMap::<usize, Connection>::new();
    let writers = HashMap::<usize, Connection>::new();
    let writers = Arc::new(Mutex::new(writers));

    // The writer
    let writer = Writer::new(writers.clone(), poller.clone());
    let data_writer_tx = writer.tx.clone();
    let subs_writer_tx = writer.tx.clone();
    let poll_writer_tx = writer.tx.clone();
    thread::spawn(move || writer.handle());

    // Subs
    let mut subs = Subs::new(subs_writer_tx);
    let subs_tx = subs.tx.clone();
    let data_subs_tx = subs.tx.clone();
    thread::spawn(move || subs.handle());

    // Data & DB
    let data = Data::new(data_writer_tx, data_subs_tx);
    let data_map = data.map.clone();
    let data_tx = data.tx.clone();

    let mut db = DB::new(data_map);
    let db_modified = db.modified.clone();
    db.load_from_file();

    thread::spawn(move || db.handle(3));
    thread::spawn(move || data.handle(db_modified));

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
                    let write_socket = read_socket.try_clone().unwrap();

                    println!("Connection #{} from {}", id, addr);

                    // Register the reading socket for events.
                    poller.add(&read_socket, Event::readable(id))?;
                    readers.insert(id, Connection::new(id, read_socket, addr));

                    // Register the writing socket for events.
                    poller.add(&write_socket, Event::none(id))?;
                    writers
                        .lock()
                        .unwrap()
                        .insert(id, Connection::new(id, write_socket, addr));

                    // One more.
                    id += 1;

                    // The server continues listening for more clients, always 0.
                    poller.modify(&server, Event::readable(0))?;
                }

                id if ev.readable => {
                    if let Some(conn) = readers.get_mut(&id) {
                        handle_reading(conn);

                        poller.modify(&conn.socket, Event::readable(id))?;

                        // One at the time.
                        if !conn.received.is_empty() {
                            let received = conn.received.remove(0);

                            // Instructions should be string.
                            if let Ok(utf8) = from_utf8(&received) {
                                // We assume multiple instructions separated with newlines.
                                for batched in utf8.trim().split('\n') {
                                    let text = batched.trim();

                                    let msg = parse(text);
                                    let instr = msg.instr;
                                    let key = msg.key;
                                    let value = msg.value;

                                    match instr {
                                        // Instructions that doesn't make sense without key.
                                        _ if key.is_empty() && needs_key(&instr) => {
                                            poll_writer_tx
                                                .send(writer::Cmd::Write(conn.id, KEY.into()))
                                                .unwrap();
                                        }

                                        // Nop
                                        Instr::Nop => {
                                            poll_writer_tx
                                                .send(writer::Cmd::Write(conn.id, NOP.into()))
                                                .unwrap();
                                        }

                                        // Set
                                        Instr::Set => {
                                            subs_tx
                                                .send(subs::Cmd::Call(
                                                    key.to_owned(),
                                                    value.to_owned(),
                                                ))
                                                .unwrap();

                                            data_tx.send(data::Cmd::Set(key, value)).unwrap();

                                            poll_writer_tx
                                                .send(writer::Cmd::Write(conn.id, OK.into()))
                                                .unwrap();
                                        }

                                        // Set only if the key doesn't exists.
                                        Instr::SetIfNone => {
                                            data_tx.send(data::Cmd::SetIfNone(key, value)).unwrap();

                                            poll_writer_tx
                                                .send(writer::Cmd::Write(conn.id, OK.into()))
                                                .unwrap();
                                        }

                                        // Makes the value an integer and increase it in 1.
                                        Instr::Inc => {
                                            data_tx.send(data::Cmd::Inc(key, conn.id)).unwrap();
                                        }

                                        // Appends the value.
                                        Instr::Append => {
                                            data_tx
                                                .send(data::Cmd::Append(key, value, conn.id))
                                                .unwrap();
                                        }

                                        // Delete!
                                        Instr::Delete => {
                                            data_tx.send(data::Cmd::Delete(key)).unwrap();

                                            poll_writer_tx
                                                .send(writer::Cmd::Write(conn.id, OK.into()))
                                                .unwrap();
                                        }

                                        // Get
                                        Instr::Get => {
                                            data_tx.send(data::Cmd::Get(key, conn.id)).unwrap();
                                        }

                                        // "Bite" query, 0x0 separated key value enumeration: key value'\0x0'key2 value2
                                        Instr::Bite => {
                                            data_tx.send(data::Cmd::Bite(key, conn.id)).unwrap();
                                        }

                                        // Trimmed Json (just the data).
                                        Instr::Jtrim => {
                                            data_tx.send(data::Cmd::Jtrim(key, conn.id)).unwrap();
                                        }

                                        // Json (full path).
                                        Instr::Json => {
                                            data_tx.send(data::Cmd::Json(key, conn.id)).unwrap();
                                        }

                                        // A generic "bite" subscription. Subscribers also receive their key: "key value"
                                        // Also a first message if value is available.
                                        Instr::SubGet | Instr::SubBite | Instr::SubJ => {
                                            subs_tx
                                                .send(subs::Cmd::Add(
                                                    key.to_owned(),
                                                    conn.id,
                                                    instr,
                                                ))
                                                .unwrap();

                                            if !value.is_empty() {
                                                subs_tx.send(subs::Cmd::Call(key, value)).unwrap()
                                            }

                                            poll_writer_tx
                                                .send(writer::Cmd::Write(conn.id, OK.into()))
                                                .unwrap();
                                        }

                                        // A desubscription and a last message if value is available.
                                        Instr::Unsub => {
                                            if !value.is_empty() {
                                                subs_tx
                                                    .send(subs::Cmd::Call(key.to_owned(), value))
                                                    .unwrap();
                                            }

                                            subs_tx.send(subs::Cmd::Del(key, conn.id)).unwrap();

                                            poll_writer_tx
                                                .send(writer::Cmd::Write(conn.id, OK.into()))
                                                .unwrap();
                                        }

                                        // Calls key subscribers with the new value without data modifications.
                                        Instr::Signal => {
                                            subs_tx.send(subs::Cmd::Call(key, value)).unwrap();

                                            poll_writer_tx
                                                .send(writer::Cmd::Write(conn.id, OK.into()))
                                                .unwrap();
                                        }
                                    }

                                    println!("{}: {}", conn.addr, text);
                                }
                            }
                        }

                        // Forget it, it died.
                        if conn.closed {
                            poller.delete(&conn.socket)?;
                            readers.remove(&id);
                        }
                    }
                }

                id if ev.writable => {
                    let mut writers = writers.lock().unwrap();

                    if let Some(conn) = writers.get_mut(&id) {
                        handle_writing(conn);

                        // We need to send more.
                        if !conn.to_write.is_empty() {
                            poller
                                .modify(&conn.socket, Event::writable(conn.id))
                                .unwrap();
                        }

                        // Forget it, it died.
                        if conn.closed {
                            poller.delete(&conn.socket)?;
                            writers.remove(&id);
                        }
                    }
                }

                // Events that I don't care. Probably Event::none?
                _ => (),
            }
        }
    }
}

fn handle_reading(conn: &mut Connection) {
    let data = match read(conn) {
        Ok(data) => data,
        Err(err) => {
            println!("Connection #{} broken, read failed: {}", conn.id, err);
            conn.closed = true;
            return;
        }
    };

    conn.received.push(data);
}

fn handle_writing(conn: &mut Connection) {
    let data = conn.to_write.remove(0);

    if let Err(err) = conn.socket.write(&data) {
        println!("Connection #{} broken, write failed: {}", conn.id, err);
        conn.closed = true;
    }
}

fn read(conn: &mut Connection) -> io::Result<Vec<u8>> {
    let mut received = vec![0; 1024 * 4];
    let mut bytes_read = 0;

    loop {
        match conn.socket.read(&mut received[bytes_read..]) {
            Ok(0) => {
                // Reading 0 bytes means the other side has closed the
                // connection or is done writing, then so are we.
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "0 bytes read"));
            }
            Ok(n) => {
                bytes_read += n;
                if bytes_read == received.len() {
                    received.resize(received.len() + 1024, 0);
                }
            }
            // Would block "errors" are the OS's way of saying that the
            // connection is not actually ready to perform this I/O operation.
            // @todo Wondering if this should be a panic instead.
            Err(ref err) if would_block(err) => break,
            Err(ref err) if interrupted(err) => continue,
            // Other errors we'll consider fatal.
            Err(err) => return Err(err),
        }
    }

    // let received_data = &received_data[..bytes_read]; // @doubt Using this
    // slice thing and returning with into() versus using the resize? Hm.

    received.resize(bytes_read, 0);

    Ok(received)
}

fn would_block(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::WouldBlock
}

fn interrupted(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::Interrupted
}
