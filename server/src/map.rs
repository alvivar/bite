use crossbeam_channel::{unbounded, Receiver, Sender};

use serde_json::{self, json};

use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use crate::{parse, subs};

pub enum Command {
    Get(Sender<Result>, String),
    Set(String, String),
    SetIfNone(String, String, Sender<subs::Command>),
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

        let (sender, receiver) = unbounded();

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
                    drop(&map);

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

                    if let Err(_) = conn_sender.send(Result::Message(msg)) {
                        subs_sender
                            .send(subs::Command::Clean(conn_sender, key))
                            .unwrap();
                    }
                }

                Command::Json(conn_sender, key) => {
                    let map = self.data.lock().unwrap();
                    let range = map.range(key.to_owned()..);
                    drop(&map);

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

                    if let Err(_) = conn_sender.send(Result::Message(msg)) {
                        subs_sender
                            .send(subs::Command::Clean(conn_sender, key))
                            .unwrap();
                    }
                }

                Command::Get(conn_sender, key) => {
                    let map = self.data.lock().unwrap();
                    let msg = match map.get(&key) {
                        Some(val) => val,
                        None => "",
                    };
                    drop(&map);

                    if let Err(_) = conn_sender.send(Result::Message(msg.to_owned())) {
                        subs_sender
                            .send(subs::Command::Clean(conn_sender, key))
                            .unwrap();
                    }
                }

                Command::Set(key, val) => {
                    let mut map = self.data.lock().unwrap();
                    map.insert(key, val);
                    drop(&map);

                    db_modified.swap(true, Ordering::Relaxed);
                }

                Command::SetIfNone(key, val, subs_sender) => {
                    let mut map = self.data.lock().unwrap();

                    match map.get(&key) {
                        Some(val) => {
                            let val = val.to_owned();
                            subs_sender.send(subs::Command::Call(key, val)).unwrap();

                            continue;
                        }

                        None => {
                            let k = key.to_owned();
                            let v = val.to_owned();
                            subs_sender.send(subs::Command::Call(k, v)).unwrap();

                            map.insert(key, val);

                            db_modified.swap(true, Ordering::Relaxed);
                        }
                    };
                }
            }
        }
    }
}
