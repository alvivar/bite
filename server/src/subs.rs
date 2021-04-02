use crossbeam_channel::{unbounded, Receiver, Sender};
use serde_json::json;

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    usize,
};

use crate::{map, parse::Instr};

pub enum Command {
    New(Sender<map::Result>, String, Instr),
    Call(String, String),
    Clean,
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

        let (sender, receiver) = unbounded();

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
                    let mut subs = self.subs.lock().unwrap();

                    for alt_key in get_key_combinations(key.as_str()) {
                        let sub_vec = match subs.get_mut(&alt_key) {
                            Some(val) => val,

                            None => continue,
                        };

                        let mut bad_senders = Vec::<usize>::new();

                        for (i, sub) in sub_vec.iter().enumerate() {
                            let instr = &sub.instr;
                            let sender = sub.sender.clone();

                            let msg = match instr {
                                Instr::SubJ => {
                                    let last = key.split(".").last().unwrap();

                                    json!({ last: val }).to_string()
                                }

                                Instr::SubGet => {
                                    // This makes the subscription precise, on
                                    // the exact subscribed key, instead of any
                                    // children modified.

                                    // if alt_key != key {
                                    //     continue;
                                    // }

                                    val.to_owned()
                                }

                                Instr::SubBite => {
                                    let last = key.split(".").last().unwrap();

                                    format!("{} {}", last, val)
                                }

                                _ => {
                                    println!("Unknown instruction calling subscribers");
                                    continue;
                                }
                            };

                            if let Err(_) = sender.send(map::Result::Message(msg)) {
                                bad_senders.push(i);
                            }
                        }

                        for &i in bad_senders.iter().rev() {
                            println!("Dropping subscription on {}", key);
                            sub_vec.swap_remove(i);
                        }
                    }
                }

                Command::Clean => {
                    let mut subs = self.subs.lock().unwrap();

                    let mut count: u32 = 0;

                    for (_, subs_vec) in subs.iter_mut() {
                        let mut orphans = Vec::<usize>::new();
                        for (i, sub) in subs_vec.iter().enumerate() {
                            if let Err(_) = sub.sender.send(map::Result::Ping) {
                                orphans.push(i);
                            }
                        }

                        for &i in orphans.iter().rev() {
                            count += 1;
                            subs_vec.swap_remove(i);
                        }
                    }

                    println!("{} orphan subscriptions removed", count);
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
