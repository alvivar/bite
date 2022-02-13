use crate::subs::{self, Cmd::Call};
use crate::writer::{self, Cmd::Write};

use crossbeam_channel::{unbounded, Receiver, Sender};
use serde_json::{self, json, Value};

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub enum Cmd {
    Set(String, String),
    SetIfNone(String, String),
    Inc(String, usize),
    Append(String, String, usize),
    Delete(String),
    Get(String, usize),
    Bite(String, usize),
    Jtrim(String, usize),
    Json(String, usize),
}

pub struct Data {
    pub map: Arc<Mutex<BTreeMap<String, String>>>,
    writer_tx: Sender<writer::Cmd>,
    subs_tx: Sender<subs::Cmd>,
    pub tx: Sender<Cmd>,
    rx: Receiver<Cmd>,
}

impl Data {
    pub fn new(writer_tx: Sender<writer::Cmd>, subs_tx: Sender<subs::Cmd>) -> Data {
        let map = Arc::new(Mutex::new(BTreeMap::<String, String>::new()));
        let (tx, rx) = unbounded::<Cmd>();

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
                Cmd::Set(key, val) => {
                    let mut map = self.map.lock().unwrap();

                    map.insert(key, val);

                    db_modified.swap(true, Ordering::Relaxed);
                }

                Cmd::SetIfNone(key, val) => {
                    let mut map = self.map.lock().unwrap();

                    if map.get(&key).is_none() {
                        self.subs_tx
                            .send(Call(key.to_owned(), val.to_owned()))
                            .unwrap();

                        map.insert(key, val);

                        db_modified.swap(true, Ordering::Relaxed);
                    }
                }

                Cmd::Inc(key, id) => {
                    let mut map = self.map.lock().unwrap();

                    let inc = match map.get(&key) {
                        Some(val) => match val.parse::<u32>() {
                            Ok(n) => n + 1,
                            Err(_) => 0,
                        },

                        None => 0,
                    };

                    self.subs_tx
                        .send(Call(key.to_owned(), inc.to_string()))
                        .unwrap();

                    self.writer_tx.send(Write(id, inc.to_string())).unwrap();

                    map.insert(key, inc.to_string());

                    db_modified.swap(true, Ordering::Relaxed);
                }

                Cmd::Append(key, val, id) => {
                    let mut map = self.map.lock().unwrap();

                    let mut append = String::new();
                    match map.get_mut(&key) {
                        Some(v) => {
                            append.push_str(v);
                            append.push_str(&val);
                        }

                        None => {
                            append = val;
                        }
                    };

                    self.subs_tx
                        .send(Call(key.to_owned(), append.to_owned()))
                        .unwrap();

                    self.writer_tx.send(Write(id, append.to_owned())).unwrap();

                    map.insert(key, append);

                    db_modified.swap(true, Ordering::Relaxed);
                }

                Cmd::Delete(key) => {
                    let mut map = self.map.lock().unwrap();

                    if map.remove(&key).is_some() {
                        db_modified.swap(true, Ordering::Relaxed);
                    }
                }

                Cmd::Get(key, id) => {
                    let map = self.map.lock().unwrap();
                    let msg = match map.get(&key) {
                        Some(val) => val,
                        None => "",
                    };

                    self.writer_tx.send(Write(id, msg.into())).unwrap();
                }

                Cmd::Bite(key, id) => {
                    let map = self.map.lock().unwrap();
                    let range = map.range(key.to_owned()..);

                    let kv: Vec<(&str, &str)> = range
                        .take_while(|(k, _)| k.starts_with(&key))
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect();

                    let mut msg = String::new();
                    for (k, v) in kv {
                        let k = k.split('.').last().unwrap();
                        msg.push_str(format!("{} {}\0", k, v).as_str());
                    }
                    let msg = msg.trim_end().to_owned(); // @todo What's happening here exactly?

                    self.writer_tx.send(Write(id, msg)).unwrap();
                }

                Cmd::Jtrim(key, id) => {
                    let map = self.map.lock().unwrap();
                    let range = map.range(key.to_owned()..);

                    let kv: Vec<(&str, &str)> = range
                        .take_while(|(k, _)| k.starts_with(&key))
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect();

                    let json = kv_to_json(&*kv);

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

                    self.writer_tx.send(Write(id, msg)).unwrap();
                }

                Cmd::Json(key, id) => {
                    let map = self.map.lock().unwrap();
                    let range = map.range(key.to_owned()..);

                    let kv: Vec<(&str, &str)> = range
                        .take_while(|(k, _)| k.starts_with(&key))
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect();

                    let json = kv_to_json(&*kv);

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

                    self.writer_tx.send(Write(id, msg)).unwrap();
                }
            }
        }
    }
}

pub fn kv_to_json(kv: &[(&str, &str)]) -> Value {
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
