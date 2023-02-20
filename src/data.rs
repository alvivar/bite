use crate::subs::{self, Action::Call};
use crate::writer::{self, Action::Queue, Order};

use crossbeam_channel::{unbounded, Receiver, Sender};
use serde_json::{self, json, Value};

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub enum Action {
    Set(String, Vec<u8>),
    SetIfNone(String, Vec<u8>, usize, usize),
    Inc(String, usize, usize),
    Append(String, Vec<u8>, usize, usize),
    Delete(String),
    Get(String, usize, usize),
    KeyValue(String, usize, usize),
    Jtrim(String, usize, usize),
    Json(String, usize, usize),
}

pub struct Data {
    pub map: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
    writer_tx: Sender<writer::Action>,
    subs_tx: Sender<subs::Action>,
    pub tx: Sender<Action>,
    rx: Receiver<Action>,
}

impl Data {
    pub fn new(writer_tx: Sender<writer::Action>, subs_tx: Sender<subs::Action>) -> Data {
        let map = Arc::new(Mutex::new(BTreeMap::<String, Vec<u8>>::new()));
        let (tx, rx) = unbounded::<Action>();

        Data {
            map,
            writer_tx,
            subs_tx,
            tx,
            rx,
        }
    }

    pub fn handle(&self, db_modified: Arc<AtomicBool>) {
        loop {
            match self.rx.recv().unwrap() {
                Action::Set(key, val) => {
                    let mut map = self.map.lock().unwrap();

                    map.insert(key, val);

                    db_modified.swap(true, Ordering::Relaxed);
                }

                Action::SetIfNone(key, val, from_id, msg_id) => {
                    let mut map = self.map.lock().unwrap();

                    if map.get(&key).is_none() {
                        self.subs_tx
                            .send(Call(key.to_owned(), val.to_owned(), from_id, msg_id))
                            .unwrap();

                        map.insert(key, val);

                        db_modified.swap(true, Ordering::Relaxed);
                    }
                }

                Action::Inc(key, from_id, msg_id) => {
                    let mut map = self.map.lock().unwrap();

                    let inc = match map.get(&key) {
                        Some(val) => vec_to_u64(val) + 1,
                        None => 1,
                    };

                    let inc_vec = u64_to_vec(inc);

                    self.writer_tx
                        .send(Queue(Order {
                            from_id,
                            to_id: from_id,
                            msg_id,
                            data: inc_vec.to_owned(),
                        }))
                        .unwrap();

                    self.subs_tx
                        .send(Call(key.to_owned(), inc_vec.to_owned(), from_id, msg_id))
                        .unwrap();

                    map.insert(key, inc_vec);

                    db_modified.swap(true, Ordering::Relaxed);
                }

                Action::Append(key, data, from_id, msg_id) => {
                    let mut map = self.map.lock().unwrap();

                    let value = match map.get_mut(&key) {
                        Some(value) => {
                            value.extend(data.to_owned());
                            value.to_owned()
                        }

                        None => {
                            let mut value = Vec::new();
                            value.extend(data.to_owned());
                            value
                        }
                    };

                    self.subs_tx
                        .send(Call(key.to_owned(), data, from_id, msg_id))
                        .unwrap();

                    map.insert(key, value);

                    db_modified.swap(true, Ordering::Relaxed);
                }

                Action::Delete(key) => {
                    let mut map = self.map.lock().unwrap();

                    if map.remove(&key).is_some() {
                        db_modified.swap(true, Ordering::Relaxed);
                    }
                }

                Action::Get(key, from_id, msg_id) => {
                    let map = self.map.lock().unwrap();

                    let message = match map.get(&key) {
                        Some(value) => value.to_owned(),
                        None => Vec::new(),
                    };

                    self.writer_tx
                        .send(Queue(Order {
                            from_id,
                            to_id: from_id,
                            msg_id,
                            data: message,
                        }))
                        .unwrap();
                }

                Action::KeyValue(key, from_id, msg_id) => {
                    let map = self.map.lock().unwrap();
                    let range = map.range(key.to_owned()..);

                    let key_value: Vec<_> = range
                        .take_while(|(k, _)| k.starts_with(&key))
                        .map(|(k, v)| (k.as_str(), v.to_owned()))
                        .collect();

                    let mut message = Vec::<u8>::new();
                    for (key, mut value) in key_value {
                        let key = key.split('.').last().unwrap();
                        message.extend(key.as_bytes());
                        message.extend(b" ");
                        message.append(&mut value);
                        message.extend(b"\0");
                    }

                    // The Rust way
                    if let Some(last) = message.iter().last() {
                        if last == &b'\0' {
                            message.pop();
                        }
                    }

                    self.writer_tx
                        .send(Queue(Order {
                            from_id,
                            to_id: from_id,
                            msg_id,
                            data: message,
                        }))
                        .unwrap();
                }

                Action::Jtrim(key, from_id, msg_id) => {
                    let map = self.map.lock().unwrap();
                    let range = map.range(key.to_owned()..);

                    let key_value: Vec<_> = range
                        .take_while(|(k, _)| k.starts_with(&key))
                        .map(|(k, v)| (k.as_str(), v))
                        .collect();

                    let json = kv_to_json(&key_value);

                    // [!] Returns the pointer.
                    // Always returns everything when the key is empty.
                    let pointr = format!("/{}", key.replace('.', "/"));
                    let message = match json.pointer(pointr.as_str()) {
                        Some(value) => value.to_string(),
                        None => {
                            let message = if pointr.len() <= 1 { json } else { json!({}) };
                            message.to_string()
                        }
                    };

                    self.writer_tx
                        .send(Queue(Order {
                            from_id,
                            to_id: from_id,
                            msg_id,
                            data: message.into(),
                        }))
                        .unwrap();
                }

                Action::Json(key, from_id, msg_id) => {
                    let map = self.map.lock().unwrap();
                    let range = map.range(key.to_owned()..);

                    let kv: Vec<_> = range
                        .take_while(|(k, _)| k.starts_with(&key))
                        .map(|(k, v)| (k.as_str(), v))
                        .collect();

                    let json = kv_to_json(&kv);

                    // [!] Returns the json, but only if the pointer is real.
                    // Always returns everything when the key is empty.
                    let pointr = format!("/{}", key.replace('.', "/"));
                    let message = match json.pointer(pointr.as_str()) {
                        Some(_) => json.to_string(),
                        None => {
                            let message = if pointr.len() <= 1 { json } else { json!({}) };
                            message.to_string()
                        }
                    };

                    self.writer_tx
                        .send(Queue(Order {
                            from_id,
                            to_id: from_id,
                            msg_id,
                            data: message.into(),
                        }))
                        .unwrap();
                }
            }
        }
    }
}

// @todo I don't really understand this, I took this code from a Discord chat
// when I asked for help. I wanted to merge json values with the same parent.
pub fn kv_to_json(kv: &[(&str, &Vec<u8>)]) -> Value {
    let mut merged_json = json!({});

    // NOTE(Wojciech): Unfinished alternative.
    // kv.iter().map(|(k, v)| k.split(".").map(|name| {}));

    for (k, v) in kv.iter().rev() {
        insert(&mut merged_json, k, json!(v));
    }

    merged_json
}

fn insert(mut json: &mut Value, key: &str, val: Value) {
    for k in key.split('.') {
        json = json
            .as_object_mut()
            .unwrap()
            .entry(k)
            .or_insert_with(|| json!({}));
    }

    if json == &json!({}) {
        *json = val;
    }
}

/// Transforms a byte array into a u64. Tries to parse from string when the size
/// isn't 64 bits, but this means that "12345678" will be considered a u64 and
/// not a string, because it has a length of 8 bytes. Pretty simple but inexact
/// rule.
fn vec_to_u64(vec: &[u8]) -> u64 {
    if vec.len() != 8 {
        let utf8 = String::from_utf8_lossy(vec);
        return utf8.parse::<u64>().unwrap_or(0);
    }

    let vec64 = vec[0..8].try_into().unwrap_or(&[0; 8]);
    u64::from_be_bytes(*vec64)
}

fn u64_to_vec(n: u64) -> Vec<u8> {
    n.to_be_bytes().to_vec()
}
