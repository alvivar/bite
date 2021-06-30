use std::{
    io::{self, Read},
    str::from_utf8,
    sync::mpsc::Sender,
};

use crate::{
    conn::Connection,
    parse::parse,
    ready,
    subs::{self},
};

pub struct Reader {
    subs_tx: Sender<subs::Cmd>,
    ready_tx: Sender<ready::Cmd>,
}

impl Reader {
    pub fn new(subs_tx: Sender<subs::Cmd>, ready_tx: Sender<ready::Cmd>) -> Reader {
        Reader { subs_tx, ready_tx }
    }

    pub fn handle(self, mut conn: Connection) {
        let data = match read(&mut conn) {
            Ok(data) => data,
            Err(err) => {
                println!("Connection #{} broken, failed read: {}", conn.id, err);
                return;
            }
        };

        // Handle the message as string.
        if let Ok(utf8) = from_utf8(&data) {
            let msg = parse(utf8);
            let op = msg.op.as_str();
            let key = msg.key;
            let val = msg.value;

            if !key.is_empty() {
                match op {
                    // A subscription and a first message.
                    "+" => {
                        self.subs_tx
                            .send(subs::Cmd::Add(key.to_owned(), conn.id))
                            .unwrap();

                        if !val.is_empty() {
                            self.subs_tx.send(subs::Cmd::Call(key, val)).unwrap()
                        }
                    }

                    // A message to subscriptions.
                    ":" => {
                        self.subs_tx.send(subs::Cmd::Call(key, val)).unwrap();
                    }

                    // A desubscription and a last message.
                    "-" => {
                        if !val.is_empty() {
                            self.subs_tx
                                .send(subs::Cmd::Call(key.to_owned(), val))
                                .unwrap();
                        }

                        self.subs_tx.send(subs::Cmd::Del(key, conn.id)).unwrap();
                    }

                    _ => (),
                }
            }

            println!("{}: {}", conn.addr, utf8.trim_end());
        }

        // Re-register the connection for more readings.
        self.ready_tx.send(ready::Cmd::Read(conn)).unwrap();
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
