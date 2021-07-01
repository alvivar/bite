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
    Get(String, Sender<String>),
    Bite(String, Sender<Result>),
    Jtrim(String, Sender<Result>),
    Json(String, Sender<Result>),
    Set(String, String),
    // SetIfNone(String, String, Sender<subs::Command>),
    // Inc(String, Sender<Result>, Sender<subs::Command>),
    // Append(String, String, Sender<Result>, Sender<subs::Command>),
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

                Cmd::Get(key, tx) => {
                    let map = self.data.lock().unwrap();
                    let msg = match map.get(&key) {
                        Some(val) => val,
                        None => "",
                    };

                    tx.send(msg.to_owned()).unwrap();
                }

                Cmd::Delete(key) => {
                    let mut map = self.data.lock().unwrap();
                    map.remove(&key);
                    drop(&map);

                    db_modified.swap(true, Ordering::Relaxed);
                }

                Cmd::Bite(key, conn_sender) => {
                    let map = self.data.lock().unwrap();
                    let range = map.range(key.to_owned()..);
                    drop(&map);

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

                    conn_sender.send(Result::Message(msg)).unwrap();
                }

                Cmd::Jtrim(key, conn_sender) => {
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

                    conn_sender.send(Result::Message(msg)).unwrap();
                }

                Cmd::Json(key, conn_sender) => {
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

                    conn_sender.send(Result::Message(msg)).unwrap();
                } // Command::SetIfNone(key, val, subs_sender) => {
                  //     let mut map = self.data.lock().unwrap();

                  //     match map.get(&key) {
                  //         Some(_) => {}

                  //         None => {
                  //             map.insert(key.to_owned(), val.to_owned());
                  //             drop(map);

                  //             subs_sender.send(subs::Command::Call(key, val)).unwrap();

                  //             db_modified.swap(true, Ordering::Relaxed);
                  //         }
                  //     };
                  // }
                  // Command::Inc(key, conn_sender, subs_sender) => {
                  //     let mut map = self.data.lock().unwrap();

                  //     let inc = match map.get(&key) {
                  //         Some(val) => match val.parse::<u32>() {
                  //             Ok(n) => n + 1,

                  //             Err(_) => 0,
                  //         },

                  //         None => 0,
                  //     };

                  //     map.insert(key.to_owned(), inc.to_string());
                  //     drop(map);

                  //     conn_sender.send(Result::Message(inc.to_string())).unwrap();

                  //     subs_sender
                  //         .send(subs::Command::Call(key, inc.to_string()))
                  //         .unwrap();

                  //     db_modified.swap(true, Ordering::Relaxed);
                  // }
                  // Command::Append(key, val, conn_sender, subs_sender) => {
                  //     let mut map = self.data.lock().unwrap();

                  //     let mut append = String::new();

                  //     match map.get_mut(&key) {
                  //         Some(v) => {
                  //             append.push_str(v.as_str());
                  //             append.push_str(val.as_str());

                  //             v.push_str(val.as_str());
                  //         }

                  //         None => {
                  //             append = val;
                  //             map.insert(key.to_owned(), append.to_owned());
                  //         }
                  //     };

                  //     drop(map);

                  //     conn_sender
                  //         .send(Result::Message(append.to_owned()))
                  //         .unwrap();

                  //     subs_sender.send(subs::Command::Call(key, append)).unwrap();

                  //     db_modified.swap(true, Ordering::Relaxed);
                  // }
            }
        }
    }
}
