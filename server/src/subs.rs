use crate::{map, parse::Instr};
use serde_json::json;
use std::{
    collections::BTreeMap,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    usize,
};

pub enum Command {
    New(Sender<map::Result>, String, Instr),
    Call(String, String),
    Clean(Sender<map::Result>, String),
}

pub struct Sub {
    sender: Sender<map::Result>,
    instr: Instr,
}

pub struct Subs {
    pub subs: Arc<Mutex<BTreeMap<String, Vec<Sub>>>>,
    pub sender: Sender<Command>,
    receiver: Receiver<Command>,
}

impl Subs {
    pub fn new() -> Subs {
        let subs = Arc::new(Mutex::new(BTreeMap::<String, Vec<Sub>>::new()));

        let (sender, receiver) = mpsc::channel();

        Subs {
            subs,
            sender,
            receiver,
        }
    }

    pub fn handle(&self) {
        loop {
            let message = self.receiver.recv().unwrap();

            match message {
                Command::New(sender, key, instr) => {
                    let mut subs = self.subs.lock().unwrap();

                    let senders = subs.entry(key).or_insert_with(Vec::new);

                    senders.push(Sub { sender, instr });
                }

                Command::Call(key, val) => {
                    let subs = self.subs.lock().unwrap();

                    for alt_key in get_key_combinations(key.as_str()) {
                        let sub_vec = match subs.get(&alt_key) {
                            Some(val) => val,
                            None => continue,
                        };

                        for sub in sub_vec {
                            let instr = &sub.instr;
                            let sender = sub.sender.clone();

                            let msg = match instr {
                                Instr::SubJ => {
                                    let last = key.split(".").last().unwrap();
                                    let msg = json!({ last.to_owned() : val.to_owned() });

                                    msg.to_string()
                                }

                                Instr::SubGet => {
                                    // This commented code makes the
                                    // subscription precise, on the exact
                                    // subscribed key, instead of any children
                                    // forkey modified.

                                    // if alt_key != key {
                                    //     continue;
                                    // }

                                    val.to_string()
                                }

                                _ => {
                                    panic!("Unknown instruction calling subscribers.");
                                }
                            };

                            if let Err(_) = sender.send(map::Result::Message(msg.to_owned())) {
                                self.sender
                                    .send(Command::Clean(sender, key.to_owned()))
                                    .unwrap();
                            }
                        }
                    }
                }

                Command::Clean(_sender, key) => {
                    let mut subs = self.subs.lock().unwrap();

                    let sub_senders = match subs.get_mut(&key) {
                        Some(val) => val,
                        None => continue,
                    };

                    // @todo Is there a way to make this better? Like compare
                    // the _sender up there somehow?

                    let mut orphans = Vec::<usize>::new();
                    for (i, sub) in sub_senders.iter().enumerate() {
                        let sendr = &sub.sender;
                        if let Err(_) = sendr.send(map::Result::Ping) {
                            orphans.push(i);
                        }
                    }

                    println!("Cleaning {} orphan subscriptions.", orphans.len());
                    for (i, &index) in orphans.iter().enumerate() {
                        let end = &index - i;
                        sub_senders.remove(end);
                    }
                }
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
