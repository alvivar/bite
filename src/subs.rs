use std::{
    collections::HashMap,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
};

use polling::{Event, Poller};
use serde_json::json;

use crate::{conn::Connection, msg::Instr, writer};

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
    writer_tx: Sender<writer::Cmd>,
    pub tx: Sender<Cmd>,
    rx: Receiver<Cmd>,
}

impl Subs {
    pub fn new(writer_tx: Sender<writer::Cmd>) -> Subs {
        let registry = HashMap::<String, Vec<Sub>>::new();
        let (tx, rx) = channel::<Cmd>();

        Subs {
            registry,
            writer_tx,
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
                            for sub in subs {
                                let msg = match sub.instr {
                                    Instr::SubGet => value.to_owned(),
                                    Instr::SubBite => {
                                        let key = key.split(".").last().unwrap();
                                        format!("{} {}", key, value)
                                    }
                                    Instr::SubJ => {
                                        let key = key.split(".").last().unwrap();
                                        json!({ key: value }).to_string()
                                    }
                                    _ => unreachable!(),
                                };

                                self.writer_tx
                                    .send(writer::Cmd::Write(sub.id, msg))
                                    .unwrap();
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
