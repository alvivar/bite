use std::{
    collections::HashMap,
    io::{self, Read},
    str::from_utf8,
    sync::{Arc, Mutex},
};

use crossbeam_channel::{unbounded, Receiver, Sender};

use crate::{
    conn::Connection,
    map,
    parse::proc_from_string,
    ready,
    subs::{self},
    Work,
};

const OK: &str = "OK";
const NOP: &str = "OK";

pub struct Reader {
    write_map: Arc<Mutex<HashMap<usize, Connection>>>,
    map_tx: Sender<map::Cmd>,
    work_tx: Sender<Work>,
    subs_tx: Sender<subs::Cmd>,
    ready_tx: Sender<ready::Cmd>,
    tx: Sender<String>,
    rx: Receiver<String>,
}

impl Reader {
    pub fn new(
        write_map: Arc<Mutex<HashMap<usize, Connection>>>,
        map_tx: Sender<map::Cmd>,
        work_tx: Sender<Work>,
        subs_tx: Sender<subs::Cmd>,
        ready_tx: Sender<ready::Cmd>,
    ) -> Reader {
        let (tx, rx) = unbounded::<String>();

        Reader {
            write_map,
            map_tx,
            work_tx,
            subs_tx,
            ready_tx,
            tx,
            rx,
        }
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
            let proc = proc_from_string(utf8);
            let instr = proc.instr;
            let key = proc.key;
            let val = proc.value;

            match instr {
                crate::parse::Instr::Nop => {
                    if let Some(conn) = self.write_map.lock().unwrap().remove(&conn.id) {
                        self.work_tx
                            .send(Work::WriteVal(conn, NOP.to_owned()))
                            .unwrap();
                    }
                }

                crate::parse::Instr::Set => {
                    self.map_tx.send(map::Cmd::Set(key, val)).unwrap();

                    if let Some(conn) = self.write_map.lock().unwrap().remove(&conn.id) {
                        self.work_tx
                            .send(Work::WriteVal(conn, OK.to_owned()))
                            .unwrap();
                    }
                }

                crate::parse::Instr::Get => {
                    if let Some(conn) = self.write_map.lock().unwrap().remove(&conn.id) {
                        self.map_tx.send(map::Cmd::Get(key, self.tx)).unwrap();

                        let val = self.rx.recv().unwrap();

                        self.work_tx.send(Work::WriteVal(conn, val)).unwrap();
                    }
                }

                crate::parse::Instr::SetIfNone => {
                    self.map_tx.send(map::Cmd::SetIfNone(key, val)).unwrap();

                    if let Some(conn) = self.write_map.lock().unwrap().remove(&conn.id) {
                        self.work_tx
                            .send(Work::WriteVal(conn, OK.to_owned()))
                            .unwrap();
                    }
                }

                crate::parse::Instr::Inc => {
                    if let Some(conn) = self.write_map.lock().unwrap().remove(&conn.id) {
                        self.map_tx.send(map::Cmd::Inc(key, self.tx)).unwrap();

                        let val = self.rx.recv().unwrap();

                        self.work_tx.send(Work::WriteVal(conn, val)).unwrap();
                    }
                }

                crate::parse::Instr::Delete => {
                    self.map_tx.send(map::Cmd::Delete(key)).unwrap();

                    if let Some(conn) = self.write_map.lock().unwrap().remove(&conn.id) {
                        self.work_tx
                            .send(Work::WriteVal(conn, OK.to_owned()))
                            .unwrap();
                    }
                }

                crate::parse::Instr::Append => {
                    if let Some(conn) = self.write_map.lock().unwrap().remove(&conn.id) {
                        self.map_tx
                            .send(map::Cmd::Append(key, val, self.tx))
                            .unwrap();

                        let val = self.rx.recv().unwrap();

                        self.work_tx.send(Work::WriteVal(conn, val)).unwrap();
                    }
                }

                crate::parse::Instr::Bite => {
                    if let Some(conn) = self.write_map.lock().unwrap().remove(&conn.id) {
                        self.map_tx.send(map::Cmd::Bite(key, self.tx)).unwrap();

                        let val = self.rx.recv().unwrap();

                        self.work_tx.send(Work::WriteVal(conn, val)).unwrap();
                    }
                }

                crate::parse::Instr::Jtrim => todo!(),

                crate::parse::Instr::Json => todo!(),
                crate::parse::Instr::Signal => todo!(),
                crate::parse::Instr::SubJ => todo!(),
                crate::parse::Instr::SubGet => todo!(),
                crate::parse::Instr::SubBite => todo!(),
                // // A subscription and a first message.
                // "+" => {
                //     self.subs_tx
                //         .send(subs::Cmd::Add(key.to_owned(), conn.id))
                //         .unwrap();

                //     if !val.is_empty() {
                //         self.subs_tx.send(subs::Cmd::Call(key, val)).unwrap()
                //     }
                // }

                // // A message to subscriptions.
                // ":" => {
                //     self.subs_tx.send(subs::Cmd::Call(key, val)).unwrap();
                // }

                // // A desubscription and a last message.
                // "-" => {
                //     if !val.is_empty() {
                //         self.subs_tx
                //             .send(subs::Cmd::Call(key.to_owned(), val))
                //             .unwrap();
                //     }

                //     self.subs_tx.send(subs::Cmd::Del(key, conn.id)).unwrap();
                // }
                // _ => (),
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
