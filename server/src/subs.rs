use std::{
    collections::BTreeMap,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
};

use crate::{map, parse::Instr};

pub enum Command {
    NewSub(Sender<map::Result>, String, Instr),
    CallSubs(String),
    CleanUp(String),
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

    pub fn handle(&self, map_sender: Sender<map::Command>) {
        loop {
            let message = self.receiver.recv().unwrap();

            match message {
                Command::NewSub(sender, key, instr) => {
                    let mut subs = self.subs.lock().unwrap();

                    let senders = subs.entry(key).or_insert_with(Vec::new);

                    senders.push(Sub { sender, instr });
                }
                Command::CallSubs(key) => {
                    let mut subs = self.subs.lock().unwrap();

                    for alt_key in get_key_combinations(key.to_owned()) {
                        let sub_list = subs.entry(alt_key.to_owned()).or_insert_with(Vec::new);

                        for sub in sub_list {
                            let instr = &sub.instr;
                            let sender = vec![sub.sender.clone()];

                            // @todo Optimize sending sender batches grouped by Instr.

                            let command = match instr {
                                Instr::SubJtrim => {
                                    map::Command::Jtrim(sender.clone(), alt_key.to_owned())
                                }
                                Instr::SubJson => {
                                    map::Command::Json(sender.clone(), alt_key.to_owned())
                                }
                                Instr::SubGet => {
                                    if alt_key != key {
                                        continue;
                                    }

                                    map::Command::Get(sender.clone(), alt_key.to_owned())
                                }
                                _ => panic!("Unknown instruction calling subscribers."),
                            };

                            map_sender.send(command).unwrap();
                        }
                    }
                }
                Command::CleanUp(key) => {
                    println!("@todo Needs implementation: {}", key);

                    // @todo Clean up.
                }
            }
        }
    }
}

// "data.inner.value" -> ["data", "data.inner", "data.inner.value"]
fn get_key_combinations(key: String) -> Vec<String> {
    let keys: Vec<&str> = key.split(".").collect();

    let mut parent_keys: Vec<String> = Vec::new();

    let len = keys.len();

    for i in 0..len {
        let end = len - i;
        let str = keys[..end].join(".");
        parent_keys.push(str);
    }

    parent_keys
}
