use polling::Poller;
use serde_json::{self, json, Value};

use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
};

use crate::{parse, subs, writer};

pub enum Cmd {
    Get(String, usize),
    Bite(String, usize),
    Jtrim(String, usize),
    Json(String, usize),
    Set(String, String),
    SetIfNone(String, String),
    Inc(String, usize),
    Append(String, String, usize),
    Delete(String),
}

pub struct Data {
    data: Arc<Mutex<BTreeMap<String, String>>>,
    writer_tx: Sender<writer::Cmd>,
    pub tx: Sender<Cmd>,
    rx: Receiver<Cmd>,
}

impl Data {
    pub fn new(writer_tx: Sender<writer::Cmd>) -> Data {
        let data = Arc::new(Mutex::new(BTreeMap::<String, String>::new()));
        let (tx, rx) = channel::<Cmd>();

        Data {
            data,
            writer_tx,
            tx,
            rx,
        }
    }

    pub fn handle(&self) {
        loop {
            let msg = self.rx.recv().unwrap();

            match msg {
                Cmd::Set(key, val) => {
                    let mut map = self.data.lock().unwrap();
                    map.insert(key, val);
                }

                Cmd::SetIfNone(key, val) => {
                    let mut map = self.data.lock().unwrap();

                    match map.get(&key) {
                        Some(_) => {}
                        None => {
                            map.insert(key, val);
                        }
                    };
                }

                Cmd::Inc(key, id) => {
                    let mut map = self.data.lock().unwrap();

                    let inc = match map.get(&key) {
                        Some(val) => match val.parse::<u32>() {
                            Ok(n) => n + 1,
                            Err(_) => 0,
                        },

                        None => 0,
                    };

                    map.insert(key, inc.to_string());

                    self.writer_tx
                        .send(writer::Cmd::Write(id, inc.to_string()))
                        .unwrap();
                }

                Cmd::Append(key, val, id) => {
                    let mut map = self.data.lock().unwrap();

                    let mut append = String::new();

                    match map.get_mut(&key) {
                        Some(v) => {
                            append.push_str(v);
                            append.push_str(&val);
                        }

                        None => {
                            append = val;
                            map.insert(key, append.to_owned());
                        }
                    };

                    self.writer_tx.send(writer::Cmd::Write(id, append)).unwrap();
                }

                Cmd::Get(key, id) => {
                    let map = self.data.lock().unwrap();
                    let msg = match map.get(&key) {
                        Some(val) => val,
                        None => "",
                    };

                    self.writer_tx
                        .send(writer::Cmd::Write(id, msg.into()))
                        .unwrap();
                }

                Cmd::Bite(key, id) => {
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

                    self.writer_tx.send(writer::Cmd::Write(id, msg)).unwrap();
                }

                Cmd::Jtrim(key, id) => {
                    let map = self.data.lock().unwrap();
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

                    self.writer_tx.send(writer::Cmd::Write(id, msg)).unwrap();
                }

                Cmd::Json(key, id) => {
                    let map = self.data.lock().unwrap();
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

                    self.writer_tx.send(writer::Cmd::Write(id, msg)).unwrap();
                }

                Cmd::Delete(key) => {
                    let mut map = self.data.lock().unwrap();
                    map.remove(&key);
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

    return merged_json;
}

fn insert(mut json: &mut Value, key: &str, val: Value) {
    for k in key.split('.') {
        json = json
            .as_object_mut()
            .unwrap()
            .entry(k)
            .or_insert_with(|| json!({}));
    }

    if **&json == json!({}) {
        *json = val;
    }
}
