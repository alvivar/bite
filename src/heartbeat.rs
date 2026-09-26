use std::{
    collections::HashMap,
    sync::{mpsc::Sender, Arc, Mutex},
    thread::sleep,
    time::Duration,
};

use crate::cleaner;
use crate::connection::Connection;
use crate::writer::{self, Action::QueueAll, Order};

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

    pub fn handle(&self, writer_tx: Sender<writer::Action>, cleaner_tx: Sender<cleaner::Action>) {
        loop {
            sleep(Duration::from_secs(TIMEOUT_30));
            self.drop_idle_readers(&cleaner_tx);

            sleep(Duration::from_secs(TIMEOUT_30));
            self.ping_idle_writers(&writer_tx);
        }
    }

    /// Drops readers that stopped sending bytes in the middle of a message.
    /// Closed readers were already sent to the cleaner by whoever closed them.
    fn drop_idle_readers(&self, cleaner_tx: &Sender<cleaner::Action>) {
        let mut readers = self.readers.lock().unwrap();

        for (id, connection) in readers.iter_mut() {
            let elapsed = connection.last_read.elapsed().as_secs();
            if !connection.closed && connection.messages.is_incomplete() && elapsed > TIMEOUT_30 {
                connection.closed = true;
                cleaner_tx.send(cleaner::Action::Drop(*id)).unwrap();

                info!("Dropping Reader #{id}, incomplete message timed out");
            }
        }
    }

    fn ping_idle_writers(&self, writer_tx: &Sender<writer::Action>) {
        let mut messages = Vec::<Order>::new();
        let writers = self.writers.lock().unwrap();

        for (id, connection) in writers.iter() {
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
