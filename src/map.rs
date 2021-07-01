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

pub enum Result {
    Message(String),
}

pub enum Cmd {
    Set(String, String),
    SetIfNone(String, String),
    Inc(String, Sender<String>),
    Append(String, String, Sender<String>),
    Get(String, Sender<String>),
    Bite(String, Sender<String>),
    Jtrim(String, Sender<String>),
    Json(String, Sender<String>),
    Delete(String),
}

pub struct Map {
    pub data: Arc<Mutex<BTreeMap<String, String>>>,
    pub tx: Sender<Cmd>,
    rx: Receiver<Cmd>,
}

impl Map {
    pub fn new() -> Map {
        let data = Arc::new(Mutex::new(BTreeMap::<String, String>::new()));
        let (tx, rx) = unbounded();
        Map { data, tx, rx }
    }

    pub fn handle(&self, db_modified: Arc<AtomicBool>) {
        loop {
            let cmd = self.rx.recv().unwrap();

            match cmd {
                Cmd::Set(key, val) => {
                    self.data.lock().unwrap().insert(key, val);
                    db_modified.swap(true, Ordering::Relaxed);
                }

                Cmd::SetIfNone(key, val) => {
                    let mut map = self.data.lock().unwrap();
                    if let None = map.get(&key) {
                        map.insert(key.to_owned(), val.to_owned());
                        db_modified.swap(true, Ordering::Relaxed);
                    }
                }

                Cmd::Inc(key, tx) => {
                    let mut map = self.data.lock().unwrap();

                    let inc = match map.get(&key) {
                        Some(val) => match val.parse::<u32>() {
                            Ok(n) => n + 1,
                            Err(_) => 0,
                        },
                        None => 0,
                    };

                    map.insert(key.to_owned(), inc.to_string());

                    tx.send(inc.to_string()).unwrap();

                    db_modified.swap(true, Ordering::Relaxed);
                }

                Cmd::Append(key, val, tx) => {
                    let mut map = self.data.lock().unwrap();

                    let mut append = String::new();

                    match map.get_mut(&key) {
                        Some(v) => {
                            append.push_str(v.as_str());
                            append.push_str(val.as_str());

                            v.push_str(val.as_str());
                        }

                        None => {
                            append = val;
                            map.insert(key.to_owned(), append.to_owned());
                        }
                    };

                    tx.send(append.to_owned()).unwrap();

                    db_modified.swap(true, Ordering::Relaxed);
                }

                Cmd::Get(key, tx) => {
                    let map = self.data.lock().unwrap();
                    let msg = match map.get(&key) {
                        Some(val) => val,
                        None => "",
                    };

                    tx.send(msg.to_owned()).unwrap();
                }

                Cmd::Bite(key, tx) => {
                    let map = self.data.lock().unwrap();
                    let range = map.range(key.to_owned()..);

                    let kv: Vec<(&str, &str)> = range
                        .take_while(|(k, _)| k.starts_with(&key))
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect();

                    let mut msg = String::new();
                    for (k, v) in kv {
                        let k = k.split(".").last().unwrap();
                        msg.push_str(format!("{} {}\0", k, v).as_str());
                    }
                    let msg = msg.trim_end().to_owned(); // @todo What's happening here exactly?

                    tx.send(msg).unwrap();
                }

                Cmd::Jtrim(key, tx) => {
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

                    tx.send(msg).unwrap();
                }

                Cmd::Json(key, tx) => {
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

                    tx.send(msg).unwrap();
                }

                Cmd::Delete(key) => {
                    self.data.lock().unwrap().remove(&key);
                    db_modified.swap(true, Ordering::Relaxed);
                }
            }
        }
    }
}
