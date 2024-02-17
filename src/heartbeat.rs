use std::{
    collections::HashMap,
    net::Shutdown,
    sync::{mpsc::Sender, Arc, RwLock},
    thread::sleep,
    time::Duration,
};

use crate::connection::Connection;
use crate::writer::{self, Action::QueueAll, Order};

const TIMEOUT_30: u64 = 30;
const TIMEOUT_60: u64 = 60;

pub struct Heartbeat {
    readers: Arc<RwLock<HashMap<usize, Connection>>>,
    writers: Arc<RwLock<HashMap<usize, Connection>>>,
}

impl Heartbeat {
    pub fn new(
        readers: Arc<RwLock<HashMap<usize, Connection>>>,
        writers: Arc<RwLock<HashMap<usize, Connection>>>,
    ) -> Heartbeat {
        Heartbeat { readers, writers }
    }

    pub fn handle(&self, writer_tx: Sender<writer::Action>) {
        loop {
            sleep(Duration::from_secs(TIMEOUT_30));
            self.drop_idle_readers();

            sleep(Duration::from_secs(TIMEOUT_30));
            self.ping_idle_writers(&writer_tx);
        }
    }

    fn drop_idle_readers(&self) {
        let mut readers = self.readers.write().unwrap();

        for (id, connection) in readers.iter_mut() {
            let elapsed = connection.last_read.elapsed().as_secs();
            if connection.pending_read && elapsed > TIMEOUT_30 {
                connection.closed = true;
                connection.socket.shutdown(Shutdown::Both).unwrap();

                info!("Shutting down Reader #{id}, timed out");
            }
        }
    }

    fn ping_idle_writers(&self, writer_tx: &Sender<writer::Action>) {
        let mut messages = Vec::<Order>::new();
        let mut writers = self.writers.write().unwrap();

        for (id, connection) in writers.iter_mut() {
            if connection.last_write.elapsed().as_secs() > TIMEOUT_60 {
                messages.push(Order {
                    from_id: 0,
                    to_id: *id,
                    msg_id: 0,
                    data: [].into(),
                });

                info!("PING sent to Connection #{id}");
            }
        }

        drop(writers);

        if !messages.is_empty() {
            writer_tx.send(QueueAll(messages)).unwrap();
        }
    }
}
