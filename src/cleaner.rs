use crate::connection::Connection;
use crate::subs::{self, Action::DelAll};

use crossbeam_channel::{unbounded, Receiver, Sender};
use polling::Poller;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub enum Action {
    Drop(usize),
}

pub struct Cleaner {
    poller: Arc<Poller>,
    readers: Arc<Mutex<HashMap<usize, Connection>>>,
    writers: Arc<Mutex<HashMap<usize, Connection>>>,
    used_ids: Arc<Mutex<Vec<usize>>>,
    pub tx: Sender<Action>,
    rx: Receiver<Action>,
}

impl Cleaner {
    pub fn new(
        poller: Arc<Poller>,
        readers: Arc<Mutex<HashMap<usize, Connection>>>,
        writers: Arc<Mutex<HashMap<usize, Connection>>>,
        used_ids: Arc<Mutex<Vec<usize>>>,
    ) -> Cleaner {
        let (tx, rx) = unbounded::<Action>();

        Cleaner {
            poller,
            writers,
            readers,
            used_ids,
            tx,
            rx,
        }
    }

    pub fn handle(&self, subs_tx: Sender<subs::Action>) {
        loop {
            match self.rx.recv().unwrap() {
                Action::Drop(id) => {
                    if let Some(reader) = self.readers.lock().unwrap().remove(&id) {
                        self.poller.delete(&reader.socket).unwrap();
                    }

                    if let Some(writer) = self.writers.lock().unwrap().remove(&id) {
                        self.poller.delete(&writer.socket).unwrap();
                    }

                    let mut used_ids = self.used_ids.lock().unwrap();
                    if !used_ids.contains(&id) {
                        used_ids.push(id);
                    }

                    subs_tx.send(DelAll(id)).unwrap();
                }
            }
        }
    }
}
