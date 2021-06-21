use crossbeam_channel::unbounded;
use mio::net::TcpListener;
use mio::{Events, Interest, Poll, Token};

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::str::from_utf8;
use std::sync::{Arc, Mutex};
use std::thread;

use env_logger; // @todo

mod connection;
mod db;
mod map;
mod parse;
mod pool;
mod subs;

use connection::Connection;
use db::DB;
use parse::{AsyncInstr, Instr};
use pool::ThreadPool;

fn would_block(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::WouldBlock
}

fn interrupted(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::Interrupted
}

fn next(current: &mut Token) -> Token {
    let next = current.0;
    current.0 += 1;

    Token(next)
}

fn main() -> io::Result<()> {
    env_logger::init();

    // Create a poll instance,
    let mut poll = Poll::new()?;
    // and a storage for events.
    let mut events = Events::with_capacity(1024);

    // Setup the TCP server socket.
    let addr = "0.0.0.0:1984".parse().unwrap();
    let mut server = TcpListener::bind(addr)?;

    // Register the server with poll to receive events for it.
    const SERVER: Token = Token(0);
    poll.registry()
        .register(&mut server, SERVER, Interest::READABLE)?;

    // Map of `Token` -> `TcpStream`.
    let mut connections = HashMap::<Token, Connection>::new();

    // Unique token for each incoming connection.
    let mut unique_token = Token(SERVER.0 + 1);

    // The data Map to get / set information.
    let map = map::Map::new();
    let map_tx = map.tx.clone();

    // The persistent JSON db.
    let mut db = DB::new(map.data.clone());
    let db_modified = db.modified.clone();
    db.load_from_file();

    // Subscriptions
    let subs = subs::Subs::new();
    let subs_tx = subs.tx.clone();
    // let clean_subs_tx = subs_tx.clone();

    // A thread pool handles each connection IO operations with these channels.
    let (work_tx, work_rx) = unbounded::<Connection>();
    let work_rx = Arc::new(Mutex::new(work_rx));

    // When the work is done, we use this channel to reregister IO events.
    let (ready_tx, ready_rx) = unbounded::<Connection>();

    // Threads for the btree map query, the persistent db file, and
    // subscriptions.
    thread::spawn(move || map.handle(db_modified));
    thread::spawn(move || db.handle(3));
    thread::spawn(move || subs.handle());

    let mut pool = ThreadPool::new(4);
    for _ in 0..pool.size() {
        let pool_rx = work_rx.clone();
        let ready_tx = ready_tx.clone();

        let map_tx = map_tx.clone();
        let (res_tx, res_rx) = unbounded::<String>();

        let subs_tx = subs_tx.clone();

        // Waiting for work!
        pool.submit(move || {
            loop {
                let mut conn = pool_rx.lock().unwrap().recv().unwrap();

                // First read, then write.

                // We can (maybe) read from the connection.
                println!("Trying to read");

                let mut received_data = vec![0; 4096];
                let mut bytes_read = 0;

                loop {
                    match conn.socket.read(&mut received_data[bytes_read..]) {
                        Ok(0) => {
                            // Reading 0 bytes means the other side has closed
                            // the connection or is done writing, then so are
                            // we.
                            conn.open = false;
                            break;
                        }
                        Ok(n) => {
                            bytes_read += n;
                            if bytes_read == received_data.len() {
                                received_data.resize(received_data.len() + 1024, 0);
                            }
                        }
                        // Would block "errors" are the OS's way of saying that
                        // the connection is not actually ready to perform this
                        // I/O operation.
                        Err(ref err) if would_block(err) => break,
                        Err(ref err) if interrupted(err) => {
                            println!("Interrupted!");
                            continue;
                        }
                        // Other errors we'll consider fatal.
                        Err(err) => {
                            let id = conn.token.0;
                            let addr = conn.address;
                            println!("Error with connection {} to {}: {}", id, addr, err);
                            break;
                        }
                    }
                }

                if bytes_read != 0 {
                    let received_data = &received_data[..bytes_read];
                    if let Ok(str_utf8) = from_utf8(received_data) {
                        // Data received. This is a good place to parse and
                        // respond accordingly.

                        // Parse the message
                        let proc = parse::proc_from(str_utf8);
                        let instr = proc.instr;
                        let key = proc.key;
                        let val = proc.value;

                        println!("Received data: {}", str_utf8.trim_end());
                        println!("Instr: {:?}\nKey: {}\nValue: {}", instr, key, val);

                        let tx = res_tx.clone();
                        let async_instr = match instr {
                            Instr::Nop => AsyncInstr::Nop("NOP".to_owned()),

                            Instr::Set => {
                                if key.len() < 1 {
                                    AsyncInstr::Nop("KEY?".to_owned())
                                } else {
                                    map_tx
                                        .send(map::Command::Set(key.to_owned(), val.to_owned()))
                                        .unwrap();

                                    subs_tx.send(subs::Command::Call(key, val)).unwrap();

                                    AsyncInstr::Nop("OK".to_owned())
                                }
                            }

                            Instr::SetIfNone => {
                                if key.len() < 1 {
                                    AsyncInstr::Nop("KEY?".to_owned())
                                } else {
                                    map_tx
                                        .send(map::Command::SetIfNone(
                                            key.to_owned(),
                                            val.to_owned(),
                                            subs_tx.clone(),
                                        ))
                                        .unwrap();

                                    // ^ Subscription resolves after map operation.

                                    AsyncInstr::Nop("OK".to_owned())
                                }
                            }

                            Instr::Inc => {
                                if key.len() < 1 {
                                    AsyncInstr::Nop("KEY?".to_owned())
                                } else {
                                    map_tx
                                        .send(map::Command::Inc(key, tx, subs_tx.clone()))
                                        .unwrap();

                                    // ^ Subscription resolves after map operation.

                                    AsyncInstr::Yes
                                }
                            }

                            Instr::Append => {
                                if key.len() < 1 {
                                    AsyncInstr::Nop("KEY?".to_owned())
                                } else {
                                    map_tx
                                        .send(map::Command::Append(key, val, tx, subs_tx.clone()))
                                        .unwrap();

                                    // ^ Subscription resolves after the map operation.

                                    AsyncInstr::Yes
                                }
                            }

                            Instr::Delete => {
                                if key.len() < 1 {
                                    AsyncInstr::Nop("KEY?".to_owned())
                                } else {
                                    map_tx.send(map::Command::Delete(key)).unwrap();

                                    AsyncInstr::Nop("OK".to_owned())
                                }
                            }

                            Instr::Get => {
                                if key.len() < 1 {
                                    AsyncInstr::Nop("KEY?".to_owned())
                                } else {
                                    map_tx.send(map::Command::Get(key, tx)).unwrap();
                                    AsyncInstr::Yes
                                }
                            }

                            Instr::Bite => {
                                map_tx.send(map::Command::Bite(key, tx)).unwrap();
                                AsyncInstr::Yes
                            }

                            Instr::Jtrim => {
                                map_tx.send(map::Command::Jtrim(key, tx)).unwrap();
                                AsyncInstr::Yes
                            }

                            Instr::Json => {
                                map_tx.send(map::Command::Json(key, tx)).unwrap();
                                AsyncInstr::Yes
                            }

                            Instr::Signal => {
                                if key.len() < 1 {
                                    AsyncInstr::Nop("KEY?".to_owned())
                                } else {
                                    subs_tx.send(subs::Command::Call(key, val)).unwrap();
                                    AsyncInstr::Nop("OK".to_owned())
                                }
                            }

                            Instr::SubJ | Instr::SubGet | Instr::SubBite => {
                                let (sub_tx, sub_rx) = unbounded::<subs::Result>();

                                subs_tx
                                    .send(subs::Command::New(sub_tx, key, instr))
                                    .unwrap();

                                loop {
                                    let res = match sub_rx.recv().unwrap() {
                                        subs::Result::Message(msg) => msg,
                                    };

                                    let res = res.trim_end();
                                    conn.to_send.append(&mut res.into());
                                    conn.to_send.push(0xA);

                                    println!("Trying to write");
                                    if conn.to_send.len() > 0 {
                                        println!("Writing: {:?}", &conn.to_send);

                                        // We can (maybe) write to the connection.
                                        match conn.socket.write(&conn.to_send) {
                                            // We want to write the entire `DATA` buffer in a
                                            // single go. If we write less we'll return a short
                                            // write error (same as `io::Write::write_all` does).
                                            Ok(n) if n < conn.to_send.len() => {
                                                let id = conn.token.0;
                                                let addr = conn.address;
                                                println!(
                                                    "WriteZero error with connection {} to {}",
                                                    id, addr,
                                                );
                                                // println!("WriteZero error with connection");
                                                break;
                                            }
                                            Ok(_) => {
                                                // After we've written something we'll reregister
                                                // the connection to only respond to readable
                                                // events, and clear the information to send buffer.
                                                conn.to_send.clear();
                                            }
                                            // Would block "errors" are the OS's way of saying that
                                            // the connection is not actually ready to perform this
                                            // I/O operation.
                                            Err(ref err) if would_block(err) => {}
                                            // Got interrupted (how rude!), we'll try again.
                                            Err(ref err) if interrupted(err) => {
                                                // @todo Retry, old:
                                                // return handle_connection_event(registry, connection, event)
                                            }
                                            // Other errors we'll consider fatal.
                                            Err(err) => {
                                                let id = conn.token.0;
                                                let addr = conn.address;
                                                println!(
                                                    "Error with connection {} to {}: {}",
                                                    id, addr, err
                                                );
                                                // println!("Error with connection: {}", err);
                                                break;
                                            }
                                        }
                                    }
                                }

                                continue;
                            }
                        };

                        let res = match async_instr {
                            AsyncInstr::Yes => res_rx.recv().unwrap(),
                            AsyncInstr::Nop(s) => s,
                        };

                        let res = res.trim_end();
                        conn.to_send.append(&mut res.into());
                        conn.to_send.push(0xA);
                    } else {
                        println!("Received (none UTF-8) data: {:?}", received_data);
                        println!("Ignoring ^ for the moment.");
                    }
                }

                println!("Trying to write");
                if conn.to_send.len() > 0 {
                    println!("Writing: {:?}", &conn.to_send);

                    // We can (maybe) write to the connection.
                    match conn.socket.write(&conn.to_send) {
                        // We want to write the entire `DATA` buffer in a
                        // single go. If we write less we'll return a short
                        // write error (same as `io::Write::write_all` does).
                        Ok(n) if n < conn.to_send.len() => {
                            let id = conn.token.0;
                            let addr = conn.address;
                            println!("WriteZero error with connection {} to {}", id, addr,);
                            // println!("WriteZero error with connection");
                            break;
                        }
                        Ok(_) => {
                            // After we've written something we'll reregister
                            // the connection to only respond to readable
                            // events, and clear the information to send buffer.
                            conn.to_send.clear();
                        }
                        // Would block "errors" are the OS's way of saying that
                        // the connection is not actually ready to perform this
                        // I/O operation.
                        Err(ref err) if would_block(err) => {}
                        // Got interrupted (how rude!), we'll try again.
                        Err(ref err) if interrupted(err) => {
                            // @todo Retry, old:
                            // return handle_connection_event(registry, connection, event)
                        }
                        // Other errors we'll consider fatal.
                        Err(err) => {
                            let id = conn.token.0;
                            let addr = conn.address;
                            println!("Error with connection {} to {}: {}", id, addr, err);
                            // println!("Error with connection: {}", err);
                            break;
                        }
                    }
                }

                // @todo Working on the map channel ^ up there, we probably
                // don't need this below.

                // Let's reregister the connection for more IO events.
                println!("ready_tx on complete cycle!");
                ready_tx.send(conn).unwrap();
            }
        });
    }

    // Simple to test.
    println!("You can connect to the server using 'nc':");
    println!(" $ nc 127.0.0.1 1984");
    println!("Send a message to receive the same message.");

    // IO events.
    loop {
        poll.poll(&mut events, None)?;

        for event in events.iter() {
            match event.token() {
                SERVER => loop {
                    // Received an event for the TCP server socket, which
                    // indicates we can accept an connection.
                    let (mut socket, address) = match server.accept() {
                        Ok((connection, address)) => (connection, address),
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            // If we get a `WouldBlock` error we know our
                            // listener has no more incoming connections queued,
                            // so we can return to polling and wait for some
                            // more.
                            break;
                        }
                        Err(e) => {
                            // If it was any other kind of error, something went
                            // wrong and we terminate with an error.
                            return Err(e);
                        }
                    };

                    println!("Accepted connection from: {}", address);

                    let token = next(&mut unique_token);
                    poll.registry().register(
                        &mut socket,
                        token,
                        Interest::READABLE,
                        // Interest::WRITABLE.add(Interest::READABLE),
                    )?;

                    let conn = Connection::new(token, socket, address);
                    connections.insert(token, conn);
                },
                token => {
                    // Maybe received an event for a TCP connection.
                    if let Some(connection) = connections.remove(&token) {
                        // @todo Experiment, but better not.
                        // poll.registry().deregister(&mut connection.socket)?;

                        if event.is_readable() {
                            work_tx.send(connection).unwrap();
                        } else if event.is_writable() {
                            work_tx.send(connection).unwrap();
                        }
                    }

                    // Sporadic events happen, we can safely ignore them.
                }
            }
        }

        // Let's reregister the connection as needed.
        loop {
            let try_conn = ready_rx.try_recv();
            println!("try");

            match try_conn {
                Ok(mut conn) if !conn.open => {
                    println!("Connection {} closed", conn.token.0);
                    poll.registry().deregister(&mut conn.socket)?;
                    connections.remove(&conn.token);
                }

                Ok(mut conn) => {
                    println!("try_conn: {:?}", conn.to_send);

                    if conn.to_send.len() > 0 {
                        println!("Connection {} has something to write", conn.token.0);
                        poll.registry().reregister(
                            &mut conn.socket,
                            conn.token,
                            Interest::WRITABLE,
                        )?;
                    } else {
                        println!("Connection {} could read something", conn.token.0);
                        poll.registry().reregister(
                            &mut conn.socket,
                            conn.token,
                            Interest::READABLE,
                        )?;
                    }

                    connections.insert(conn.token, conn);
                }

                _ => {
                    println!("Unusual try_conn");
                    break;
                }
            }
        }
    }
}
