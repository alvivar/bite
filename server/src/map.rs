use crate::db;
use crate::parse;

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
    Json(Sender<Result>, String),
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
                Command::Json(handle, key) => {
                    let map = self.data.lock().unwrap();

                    let range = map.range(key.to_owned()..);
                    let kv: Vec<(&String, &String)> =
                        range.take_while(|(k, _)| k.starts_with(&key)).collect();

                    let json = parse::kv_to_json_value(kv);
                    handle.send(Result::Message(json)).unwrap();
                }
                Command::Get(handle, key) => {
                    let map = self.data.lock().unwrap();

                    match map.get(&key) {
                        Some(json) => handle.send(Result::Message(json.to_owned())).unwrap(),
                        None => handle.send(Result::Message("".to_owned())).unwrap(),
                    }
                }
                Command::Set(handle, key, value) => {
                    let mut map = self.data.lock().unwrap();

                    // let json = parse::to_json(&key, &value);

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
