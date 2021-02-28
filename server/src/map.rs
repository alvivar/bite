use serde_json::{self, json};

use crate::{parse, subs};

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
    Ping,
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

    pub fn handle(&self, db_modified: Arc<AtomicBool>, subs_sender: Sender<subs::Command>) {
        loop {
            let msg = self.receiver.recv().unwrap();

            match msg {
                Command::Jtrim(conn_sender, key) => {
                    let map = self.data.lock().unwrap();

                    let range = map.range(key.to_owned()..);

                    let kv: Vec<(&str, &str)> = range
                        .take_while(|(k, _)| k.starts_with(&key))
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect();

                    let json = parse::kv_to_json(&*kv);

                    // [!] Returns the pointer.
                    // Always returns everything when the key is empty.
                    let pointr = format!("/{}", key.replace(".", "/"));
                    let msg = match json.pointer(pointr.as_str()) {
                        Some(val) => val.to_string(),
                        None => {
                            let msg = if pointr.len() <= 1 { json } else { json!({}) };
                            msg.to_string()
                        }
                    };

                    if let Err(_) = conn_sender.send(Result::Message(msg.to_owned())) {
                        subs_sender
                            .send(subs::Command::CleanUp(conn_sender, key.to_owned()))
                            .unwrap();
                    }
                }
                Command::Json(conn_sender, key) => {
                    let map = self.data.lock().unwrap();

                    let range = map.range(key.to_owned()..);

                    let kv: Vec<(&str, &str)> = range
                        .take_while(|(k, _)| k.starts_with(&key))
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect();

                    let json = parse::kv_to_json(&*kv);

                    // [!] Returns the json, but only if the pointer is real.
                    // Always returns everything when the key is empty.
                    let pointr = format!("/{}", key.replace(".", "/"));
                    let msg = match json.pointer(pointr.as_str()) {
                        Some(_) => json.to_string(),
                        None => {
                            let msg = if pointr.len() <= 1 { json } else { json!({}) };
                            msg.to_string()
                        }
                    };

                    if let Err(_) = conn_sender.send(Result::Message(msg.to_owned())) {
                        subs_sender
                            .send(subs::Command::CleanUp(conn_sender, key.to_owned()))
                            .unwrap();
                    }
                }
                Command::Get(conn_sender, key) => {
                    let map = self.data.lock().unwrap();

                    let msg = match map.get(&key) {
                        Some(val) => val,
                        None => "",
                    };

                    if let Err(_) = conn_sender.send(Result::Message(msg.to_owned())) {
                        subs_sender
                            .send(subs::Command::CleanUp(conn_sender, key.to_owned()))
                            .unwrap();
                    }
                }
                Command::Set(key, value) => {
                    let mut map = self.data.lock().unwrap();

                    map.insert(key, value);

                    db_modified.swap(true, Ordering::Relaxed);
                }
            }
        }
    }
}
