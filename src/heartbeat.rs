use crate::connection::Connection;
use crate::writer::{self, Action::QueueAll, Order};

use std::collections::HashMap;
use std::net::Shutdown;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

const TIMEOUT_30: u64 = 30;
const TIMEOUT_60: u64 = 60;

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
        loop {
            // Readers

            sleep(Duration::from_secs(TIMEOUT_30));

            let mut readers = self.readers.lock().unwrap();
            for (id, connection) in readers.iter_mut() {
                let elapsed = connection.last_read.elapsed().as_secs();
                if connection.pending_read && elapsed > TIMEOUT_30 {
                    connection.closed = true;
                    connection.socket.shutdown(Shutdown::Both).unwrap();

                    println!("\nShutting down Reader #{id}, timed out");
                }
            }

            drop(readers);

            // Writers

            sleep(Duration::from_secs(TIMEOUT_30));

            let mut messages = Vec::<Order>::new();
            let mut writers = self.writers.lock().unwrap();
            for (id, connection) in writers.iter_mut() {
                if connection.last_write.elapsed().as_secs() > TIMEOUT_60 {
                    messages.push(Order {
                        from_id: 0,
                        to_id: *id,
                        msg_id: 0,
                        data: [].into(),
                    });

                    println!("\nPING sent to Writer #{id}");
                }
            }

            drop(writers);

            if !messages.is_empty() {
                writer_tx.send(QueueAll(messages)).unwrap();
            }
        }
    }
}
