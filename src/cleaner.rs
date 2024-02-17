use std::{
    collections::{HashMap, VecDeque},
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, RwLock,
    },
};

use crate::{
    connection::Connection,
    subs::{self, Action::DelAll},
};

use polling::Poller;

pub enum Action {
    Drop(usize),
}

pub struct Cleaner {
    poller: Arc<Poller>,
    readers: Arc<RwLock<HashMap<usize, Connection>>>,
    writers: Arc<RwLock<HashMap<usize, Connection>>>,
    used_ids: Arc<RwLock<VecDeque<usize>>>,
    pub tx: Sender<Action>,
    rx: Receiver<Action>,
}

impl Cleaner {
    pub fn new(
        poller: Arc<Poller>,
        readers: Arc<RwLock<HashMap<usize, Connection>>>,
        writers: Arc<RwLock<HashMap<usize, Connection>>>,
        used_ids: Arc<RwLock<VecDeque<usize>>>,
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
                    let reader = self.readers.write().unwrap().remove(&id);
                    if let Some(reader) = reader {
                        self.poller.delete(&reader.socket).unwrap();
                        self.used_ids.write().unwrap().push_back(id);
                        subs_tx.send(DelAll(id)).unwrap();
                    }

                    let writer = self.writers.write().unwrap().remove(&id);
                    if let Some(writer) = writer {
                        self.poller.delete(&writer.socket).unwrap();
                    }
                }
            }
        }
    }
}
