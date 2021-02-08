use crate::parse;

use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
};

pub enum Command {
    Get(Sender<Result>, String),
    Set(String, String),
    Json(Sender<Result>, String),
    Jtrim(Sender<Result>, String),
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

    pub fn handle(&self, db_modified: Arc<AtomicBool>) {
        loop {
            let message = self.receiver.recv().unwrap();

            match message {
                Command::Jtrim(conn_sender, key) => {
                    let map = self.data.lock().unwrap();

                    let range = map.range(key.to_owned()..);

                    let kv: Vec<(&str, &str)> = range
                        .take_while(|(k, _)| k.starts_with(&key))
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect();

                    let json = parse::kv_to_json(&*kv);

                    let jpointer = format!("/{}", key.replace(".", "/"));

                    let json = match json.pointer(jpointer.as_str()) {
                        Some(val) => val,
                        None => &json,
                    };

                    conn_sender.send(Result::Message(json.to_string())).unwrap();
                }
                Command::Json(conn_sender, key) => {
                    let map = self.data.lock().unwrap();

                    let range = map.range(key.to_owned()..);

                    let kv: Vec<(&str, &str)> = range
                        .take_while(|(k, _)| k.starts_with(&key))
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect();

                    let json = parse::kv_to_json(&*kv);

                    conn_sender.send(Result::Message(json.to_string())).unwrap();
                }
                Command::Get(conn_sender, key) => {
                    let map = self.data.lock().unwrap();

                    match map.get(&key) {
                        Some(json) => conn_sender.send(Result::Message(json.to_owned())).unwrap(),
                        None => conn_sender.send(Result::Message("".to_owned())).unwrap(),
                    }
                }
                Command::Set(key, value) => {
                    let mut map = self.data.lock().unwrap();

                    if key.len() < 1 {
                        continue;
                    }

                    map.insert(key, value);

                    db_modified.swap(true, Ordering::Relaxed);
                }
            }
        }
    }
}
