use crate::db;

use std::{
    collections::BTreeMap,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
};

pub enum Command {
    Get(Sender<Result>, String),
    Set(Sender<Result>, String, String),
}

pub enum Result {
    Message(String),
}

pub struct Map {
    pub data: Arc<Mutex<BTreeMap<String, String>>>,
    pub sender: Sender<Command>,
    receiver: Receiver<Command>,
}

impl Map {
    pub fn new() -> Map {
        let data = Arc::new(Mutex::new(BTreeMap::<String, String>::new()));
        let (sender, receiver) = mpsc::channel();

        Map {
            data,
            sender,
            receiver,
        }
    }

    pub fn handle(&self, db: Sender<db::Command>) {
        loop {
            let message = self.receiver.recv().unwrap();

            match message {
                Command::Get(handle, key) => {
                    let map = self.data.lock().unwrap();

                    match map.get(&key) {
                        Some(value) => handle.send(Result::Message(value.to_owned())).unwrap(),
                        None => handle.send(Result::Message("".to_owned())).unwrap(),
                    }
                }
                Command::Set(handle, key, value) => {
                    let mut map = self.data.lock().unwrap();

                    match map.insert(key, value) {
                        Some(_) => handle.send(Result::Message("OK".to_owned())).unwrap(),
                        None => handle.send(Result::Message("OK".to_owned())).unwrap(),
                    }

                    db.send(db::Command::Save).unwrap();
                }
            }
        }
    }
}
