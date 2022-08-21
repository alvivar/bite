use crate::parser::Command;
use crate::writer::{self, WriteOrder};

use crossbeam_channel::{unbounded, Receiver, Sender};
use serde_json::json;

use std::collections::HashMap;

pub enum Action {
    Add(String, usize, Command),
    Del(String, usize),
    DelAll(usize),
    Call(String, Vec<u8>, usize),
}

pub struct Sub {
    id: usize,
    command: Command,
}

pub struct Subs {
    key_subs: HashMap<String, Vec<Sub>>,
    id_keys: HashMap<usize, Vec<String>>,
    pub tx: Sender<Action>,
    rx: Receiver<Action>,
}

impl Subs {
    pub fn new() -> Subs {
        let key_subs = HashMap::<String, Vec<Sub>>::new();
        let id_keys = HashMap::<usize, Vec<String>>::new();
        let (tx, rx) = unbounded::<Action>();

        Subs {
            key_subs,
            id_keys,
            tx,
            rx,
        }
    }

    pub fn handle(&mut self, writer_tx: Sender<writer::Action>) {
        loop {
            match self.rx.recv().unwrap() {
                Action::Add(key, id, command) => {
                    let keys = self.id_keys.entry(id).or_insert_with(Vec::new);

                    if !keys.contains(&key) {
                        keys.push(key.to_owned());
                    }

                    let subs = self.key_subs.entry(key).or_insert_with(Vec::new);

                    if subs.iter().any(|x| x.id == id && x.command == command) {
                        continue;
                    } else {
                        subs.push(Sub { id, command })
                    }
                }

                Action::Del(key, id) => {
                    let subs = self.key_subs.entry(key).or_insert_with(Vec::new);
                    subs.retain(|x| x.id != id);
                }

                Action::DelAll(id) => {
                    if let Some(keys) = self.id_keys.remove(&id) {
                        for key in keys {
                            let subs = self.key_subs.entry(key).or_insert_with(Vec::new);
                            subs.retain(|x| x.id != id);
                        }
                    }
                }

                Action::Call(key, data, msg_id) => {
                    let mut messages = Vec::<WriteOrder>::new();

                    for alt_key in get_key_combinations(key.as_str()) {
                        if let Some(subs) = self.key_subs.get(&alt_key) {
                            for sub in subs {
                                let data = match sub.command {
                                    Command::SubGet => data.to_owned(),

                                    Command::SubKeyValue => {
                                        let key = key.split('.').last().unwrap();
                                        let mut message = Vec::<u8>::new();

                                        message.extend(key.as_bytes());
                                        message.extend(" ".as_bytes());
                                        message.extend(&data);
                                        message
                                    }

                                    Command::SubJson => {
                                        let key = key.split('.').last().unwrap();
                                        let message = String::from_utf8_lossy(&data);
                                        json!({ key: message }).to_string().into_bytes()
                                    }

                                    _ => unreachable!(),
                                };

                                messages.push(WriteOrder {
                                    from_id: sub.id,
                                    msg_id,
                                    data,
                                });
                            }
                        }
                    }

                    if !messages.is_empty() {
                        writer_tx.send(writer::Action::QueueAll(messages)).unwrap();
                    }
                }
            }
        }
    }
}

/// "data.inner.value" -> ["data.inner.value", "data.inner", "data"]
fn get_key_combinations(key: &str) -> Vec<String> {
    let mut parent_keys = Vec::<String>::new();

    let keys: Vec<&str> = key.split('.').collect();
    let len = keys.len();

    for i in 0..len {
        let end = len - i;
        let str = keys[..end].join(".");
        parent_keys.push(str);
    }

    parent_keys
}
