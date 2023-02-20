use crate::connection::Connection;
use crate::writer::{self, Action::QueueAll, Order};

use crossbeam_channel::Sender;

use std::collections::HashMap;
use std::net::Shutdown;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};

const READ_TICK: u64 = 10;
const WRITE_TICK: u64 = 30;

pub struct Heartbeat {
    readers: Arc<Mutex<HashMap<usize, Connection>>>,
    writers: Arc<Mutex<HashMap<usize, Connection>>>,
}

impl Heartbeat {
    pub fn new(
        readers: Arc<Mutex<HashMap<usize, Connection>>>,
        writers: Arc<Mutex<HashMap<usize, Connection>>>,
    ) -> Heartbeat {
        Heartbeat { readers, writers }
    }

    pub fn handle(&self, writer_tx: Sender<writer::Action>) {
        let mut read_ticker = Instant::now();
        let mut write_ticker = Instant::now();

        loop {
            sleep(Duration::from_secs(READ_TICK + 1));

            if read_ticker.elapsed().as_secs() > READ_TICK {
                read_ticker = Instant::now();

                let mut readers = self.readers.lock().unwrap();
                for (id, conn) in readers.iter_mut() {
                    if conn.pending_read && conn.last_read.elapsed().as_secs() > READ_TICK {
                        conn.closed = true;
                        conn.socket.shutdown(Shutdown::Both).unwrap();

                        println!("Shutting down Reader #{id}, timed out");
                    }
                }
            }

            if write_ticker.elapsed().as_secs() > WRITE_TICK {
                write_ticker = Instant::now();

                let mut messages = Vec::<Order>::new();
                let mut writers = self.writers.lock().unwrap();
                for (id, connection) in writers.iter_mut() {
                    if connection.last_write.elapsed().as_secs() > WRITE_TICK {
                        messages.push(Order {
                            from_id: 0,
                            to_id: *id,
                            msg_id: 0,
                            data: "PING".into(),
                        });

                        println!("PING sent to Writer #{id}");
                    }
                }

                if !messages.is_empty() {
                    writer_tx.send(QueueAll(messages)).unwrap();
                }
            }
        }
    }
}
