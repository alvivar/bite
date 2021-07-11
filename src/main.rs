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
mod msg;
mod subs;

use conn::Connection;
use msg::parse;
use subs::Subs;

fn main() -> io::Result<()> {
    // The server and the smol Poller.
    let server = TcpListener::bind("0.0.0.0:1984")?;
    server.set_nonblocking(true)?;

    let poller = Poller::new()?;
    poller.add(&server, Event::readable(0))?;
    let poller = Arc::new(poller);

    let mut readers = HashMap::<usize, Connection>::new();
    let writers = HashMap::<usize, Connection>::new();
    let writers = Arc::new(Mutex::new(writers));

    // Subs
    let mut subs = Subs::new(writers.clone(), poller.clone());
    let subs_tx = subs.tx.clone();
    thread::spawn(move || subs.handle());

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

                        // Parse the message.
                        if let Ok(utf8) = from_utf8(&conn.data) {
                            let msg = parse(utf8);
                            let op = msg.op.as_str();
                            let key = msg.key;
                            let val = msg.value;

                            match op {
                                // A subscription and a first message.
                                "+" => {
                                    subs_tx
                                        .send(subs::Cmd::Add(key.to_owned(), conn.id))
                                        .unwrap();

                                    if !val.is_empty() {
                                        subs_tx.send(subs::Cmd::Call(key, val)).unwrap()
                                    }
                                }

                                // A message to subscriptions.
                                ":" => {
                                    subs_tx.send(subs::Cmd::Call(key, val)).unwrap();
                                }

                                // A desubscription and a last message.
                                "-" => {
                                    if !val.is_empty() {
                                        subs_tx.send(subs::Cmd::Call(key.to_owned(), val)).unwrap();
                                    }

                                    subs_tx.send(subs::Cmd::Del(key, conn.id)).unwrap();
                                }

                                _ => (),
                            }

                            println!("{}: {}", conn.addr, utf8.trim_end());
                        }

                        // Forget it, it died.
                        if conn.closed {
                            poller.delete(&conn.socket)?;
                            readers.remove(&id);
                        }
                    }
                }

                id if ev.writable => {
                    let mut write_map = writers.lock().unwrap();
                    if let Some(conn) = write_map.get_mut(&id) {
                        handle_writing(conn);

                        // Forget it, it died.
                        if conn.closed {
                            poller.delete(&conn.socket)?;
                            write_map.remove(&id);
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
    conn.data = match read(conn) {
        Ok(data) => data,
        Err(err) => {
            println!("Connection #{} broken, read failed: {}", conn.id, err);
            conn.closed = true;
            return;
        }
    };
}

fn handle_writing(conn: &mut Connection) {
    if let Err(err) = conn.socket.write(&conn.data) {
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
