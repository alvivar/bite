use crate::connection::Connection;
use crate::subs::{self, Action::DelAll};

use polling::Poller;

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

pub enum Action {
    Drop(usize),
}

pub struct Cleaner {
    poller: Arc<Poller>,
    readers: Arc<Mutex<HashMap<usize, Connection>>>,
    writers: Arc<Mutex<HashMap<usize, Connection>>>,
    used_ids: Arc<Mutex<VecDeque<usize>>>,
    pub tx: Sender<Action>,
    rx: Receiver<Action>,
}

impl Cleaner {
    pub fn new(
        poller: Arc<Poller>,
        readers: Arc<Mutex<HashMap<usize, Connection>>>,
        writers: Arc<Mutex<HashMap<usize, Connection>>>,
        used_ids: Arc<Mutex<VecDeque<usize>>>,
    ) -> Cleaner {
        let (tx, rx) = channel::<Action>();

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
                        self.used_ids.lock().unwrap().push_back(id);
                        subs_tx.send(DelAll(id)).unwrap();
                    }

                    if let Some(writer) = self.writers.lock().unwrap().remove(&id) {
                        self.poller.delete(&writer.socket).unwrap();
                    }
                }
            }
        }
    }
}
