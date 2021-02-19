use std::{
    collections::BTreeMap,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    usize,
};

use crate::{map, parse::Instr};

pub enum Command {
    NewSub(Sender<map::Result>, String, Instr),
    CallSubs(String),
    CleanUp(Sender<map::Result>, String),
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

                    for alt_keys in get_key_combinations(key.to_owned()) {
                        let sub_list = subs.entry(alt_keys.to_owned()).or_insert_with(Vec::new);

                        for sub in sub_list {
                            let instr = &sub.instr;
                            let sender = vec![sub.sender.clone()];

                            // @todo Optimize sending sender batches grouped by Instr.

                            let command = match instr {
                                Instr::SubJtrim => {
                                    map::Command::Jtrim(sender.clone(), alt_keys.to_owned())
                                }
                                Instr::SubJson => {
                                    map::Command::Json(sender.clone(), alt_keys.to_owned())
                                }
                                Instr::SubGet => {
                                    if alt_keys != key {
                                        continue;
                                    }

                                    map::Command::Get(sender.clone(), alt_keys.to_owned())
                                }
                                _ => panic!("Unknown instruction calling subscribers."),
                            };

                            map_sender.send(command).unwrap();
                        }
                    }
                }
                Command::CleanUp(_sender, key) => {
                    let mut subs = self.subs.lock().unwrap();

                    let sub_list = subs.entry(key.to_owned()).or_insert_with(Vec::new);

                    // @todo Is there a way to make this better? Like compare
                    // the _sender up there somehow?

                    let mut idx = Vec::<usize>::new();
                    for (i, sub) in sub_list.iter().enumerate() {
                        let sendr = &sub.sender;
                        if let Err(_) = sendr.send(map::Result::Ping) {
                            idx.push(i);
                        }
                    }

                    for (i, &index) in idx.iter().enumerate() {
                        let end = &index - i;
                        sub_list.remove(end);
                    }
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
