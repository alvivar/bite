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
    NewSub(Sender<map::Result>, String, Instr),
    CallSub(String, String),
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

    pub fn handle(&self) {
        loop {
            let message = self.receiver.recv().unwrap();

            match message {
                Command::NewSub(sender, key, instr) => {
                    let mut subs = self.subs.lock().unwrap();

                    let senders = subs.entry(key).or_insert_with(Vec::new);

                    senders.push(Sub { sender, instr });
                }
                Command::CallSub(key, val) => {
                    let mut subs = self.subs.lock().unwrap();

                    for alt_key in get_key_combinations(key.to_owned()) {
                        let sub_list = subs.entry(alt_key.to_owned()).or_insert_with(Vec::new);

                        // @todo ^ This should be a simple check, instead of
                        // creating an entry for each alt key.

                        for sub in sub_list {
                            let instr = &sub.instr;
                            let sender = vec![sub.sender.clone()];

                            // @todo Optimize sending sender batches grouped by Instr.

                            let msg = match instr {
                                Instr::SubJ => {
                                    let last = key.split(".").last().unwrap();
                                    let msg = json!({ last.to_owned() : val.to_owned() });

                                    msg.to_string()
                                }
                                Instr::SubGet => {
                                    if alt_key != key {
                                        continue;
                                    }

                                    val.to_string()
                                }
                                _ => {
                                    panic!("Unknown instruction calling subscribers.");
                                }
                            };

                            for sndr in sender {
                                if let Err(_) = sndr.send(map::Result::Message(msg.to_owned())) {
                                    self.sender
                                        .send(Command::CleanUp(sndr, key.to_owned()))
                                        .unwrap();
                                }
                            }
                        }
                    }
                }
                Command::CleanUp(_sender, key) => {
                    let mut subs = self.subs.lock().unwrap();

                    let sub_senders = subs.entry(key.to_owned()).or_insert_with(Vec::new);

                    // @todo Is there a way to make this better? Like compare
                    // the _sender up there somehow?

                    let mut idx = Vec::<usize>::new();
                    for (i, sub) in sub_senders.iter().enumerate() {
                        let sendr = &sub.sender;
                        if let Err(_) = sendr.send(map::Result::Ping) {
                            idx.push(i);
                        }
                    }

                    for (i, &index) in idx.iter().enumerate() {
                        let end = &index - i;
                        sub_senders.remove(end);
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
