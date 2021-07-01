use std::io::Write;

use crossbeam_channel::Sender;

use crate::{conn::Connection, ready, subs};

pub struct Writer {
    subs_tx: Sender<subs::Cmd>,
    ready_tx: Sender<ready::Cmd>,
}

impl Writer {
    pub fn new(subs_tx: Sender<subs::Cmd>, ready_tx: Sender<ready::Cmd>) -> Writer {
        Writer { subs_tx, ready_tx }
    }

    pub fn handle_kv(self, mut conn: Connection, key: String, value: String) {
        let msg = format!("{} {}\n", key, value);

        match conn.socket.write(msg.as_bytes()) {
            Ok(_) => {
                self.ready_tx.send(ready::Cmd::Write(conn)).unwrap();
            }

            Err(err) => {
                println!("Connection #{} lost, failed write: {}", conn.id, err);
                self.subs_tx.send(subs::Cmd::Del(key, conn.id)).unwrap();
            }
        }
    }

    pub fn handle_v(self, mut conn: Connection, mut value: String) {
        value.push('\n');

        match conn.socket.write(value.as_bytes()) {
            Ok(_) => self.ready_tx.send(ready::Cmd::Write(conn)).unwrap(),
            Err(err) => println!("Connection #{} lost, failed write: {}", conn.id, err),
        }
    }
}
