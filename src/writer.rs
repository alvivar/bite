use std::{io::Write, sync::mpsc::Sender};

use crate::{conn::Connection, ready, subs};

pub struct Writer {
    subs_tx: Sender<subs::Cmd>,
    ready_tx: Sender<ready::Cmd>,
}

impl Writer {
    pub fn new(subs_tx: Sender<subs::Cmd>, ready_tx: Sender<ready::Cmd>) -> Writer {
        Writer { subs_tx, ready_tx }
    }

    pub fn handle(self, mut conn: Connection, key: String, value: String) {
        let msg = format!("{} {}", key, value);

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
}
