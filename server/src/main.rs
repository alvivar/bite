use mio::net::TcpListener;
use mio::{Events, Interest, Poll, Token};

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::str::from_utf8;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

use env_logger;

mod connection;
mod pool;

use connection::Connection;
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

    // A thread pool handles each connection IO operations with these channels.
    let (work_tx, work_rx) = channel::<Connection>();
    let work_rx = Arc::new(Mutex::new(work_rx));

    // When the work is done, we reregister with this for more IO events.
    let (ready_tx, ready_rx) = channel::<Connection>();

    let mut pool = ThreadPool::new(4);
    for _ in 0..pool.size() {
        let pool_rx = work_rx.clone();
        let ready_tx = ready_tx.clone();

        // Waiting for work!
        pool.submit(move || {
            loop {
                let mut conn = pool_rx.lock().unwrap().recv().unwrap();

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
                        Err(ref err) if interrupted(err) => continue,
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
                    if let Ok(str_buf) = from_utf8(received_data) {
                        println!("Received data: {}", str_buf.trim_end());
                    } else {
                        println!("Received (none UTF-8) data: {:?}", received_data);
                    }

                    // Data received. This is a good place to parse and respond
                    // accordingly.

                    conn.to_send.append(&mut received_data.into());
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
                            // return handle_connection_event(registry, connection, event)
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

                // Is the end?
                if !conn.open {
                    println!("Connection closed");
                }

                // Let's reregister the connection for more IO events.
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
                        Interest::WRITABLE.add(Interest::READABLE),
                    )?;

                    let conn = Connection::new(token, socket, address);
                    connections.insert(token, conn);
                },
                token => {
                    // Maybe received an event for a TCP connection.
                    if let Some(connection) = connections.remove(&token) {
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
            match try_conn {
                Ok(conn) if !conn.open => {
                    println!("Connection {} closed", conn.token.0);
                }
                Ok(mut conn) => {
                    if conn.to_send.len() > 0 {
                        println!("Connection {} has something to write", conn.token.0);
                        poll.registry()
                            .reregister(&mut conn.socket, conn.token, Interest::WRITABLE)
                            .unwrap();
                    } else {
                        println!("Connection {} could read something", conn.token.0);
                        poll.registry()
                            .reregister(&mut conn.socket, conn.token, Interest::READABLE)
                            .unwrap();
                    }

                    connections.insert(conn.token, conn);
                }
                _ => break,
            }
        }
    }
}
