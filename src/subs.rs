use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{conn::Connection, parse::Instr, Work};
use crossbeam_channel::{unbounded, Receiver, Sender};
use serde_json::json;

pub enum Cmd {
    Add(String, usize, Instr),
    Del(String, usize),
    Call(String, String),
}

pub struct Sub {
    id: usize,
    instr: Instr,
}

pub struct Subs {
    registry: HashMap<String, Vec<Sub>>,
    write_map: Arc<Mutex<HashMap<usize, Connection>>>,
    work_tx: Sender<Work>,
    pub tx: Sender<Cmd>,
    rx: Receiver<Cmd>,
}

impl Subs {
    pub fn new(write_map: Arc<Mutex<HashMap<usize, Connection>>>, work_tx: Sender<Work>) -> Subs {
        let registry = HashMap::<String, Vec<Sub>>::new();
        let (tx, rx) = unbounded::<Cmd>();

        Subs {
            registry,
            write_map,
            work_tx,
            tx,
            rx,
        }
    }

    pub fn handle(&mut self) {
        loop {
            match self.rx.recv() {
                Ok(Cmd::Add(key, id, instr)) => {
                    let subs = self.registry.entry(key).or_insert_with(Vec::new);

                    if subs.iter().any(|x| x.id == id) {
                        continue;
                    }

                    subs.push(Sub { id, instr })
                }

                Ok(Cmd::Del(key, id)) => {
                    let subs = self.registry.entry(key).or_insert_with(Vec::new);
                    subs.retain(|x| x.id != id);
                }

                Ok(Cmd::Call(key, value)) => {
                    for alt_key in get_key_combinations(key.as_str()) {
                        if let Some(subs) = self.registry.get(&alt_key) {
                            let mut write_map = self.write_map.lock().unwrap();
                            for sub in subs {
                                if let Some(conn) = write_map.remove(&sub.id) {
                                    let msg = match sub.instr {
                                        Instr::SubJ => {
                                            let last = key.split(".").last().unwrap();
                                            json!({ last: value }).to_string()
                                        }

                                        Instr::SubGet => value.to_owned(),

                                        Instr::SubBite => {
                                            let last = key.split(".").last().unwrap();
                                            format!("{} {}", last, value)
                                        }

                                        _ => {
                                            println!("Unknown instruction calling subs");
                                            continue;
                                        }
                                    };

                                    self.work_tx.send(Work::WriteVal(conn, msg)).unwrap();
                                }
                            }
                        }
                    }
                }

                Err(err) => panic!("The subs channel failed: {}", err),
            }
        }
    }
}

/// "data.inner.value" -> ["data.inner.value", "data.inner", "data"]
fn get_key_combinations(key: &str) -> Vec<String> {
    let mut parent_keys = Vec::<String>::new();

    let keys: Vec<&str> = key.split(".").collect();
    let len = keys.len();

    for i in 0..len {
        let end = len - i;
        let str = keys[..end].join(".");
        parent_keys.push(str);
    }

    parent_keys
}
