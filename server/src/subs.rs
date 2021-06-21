use crossbeam_channel::{unbounded, Receiver, Sender};
use serde_json::json;

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Instant,
    u32, u64, usize,
};

use crate::parse::Instr;

pub enum Command {
    New(Sender<Vec<u8>>, String, Instr),
    Call(String, String),
    Clean(u64),
}

pub struct Sub {
    sender: Sender<Vec<u8>>,
    instr: Instr,
    last_time: Instant,
}

pub struct Subs {
    pub subs: Arc<Mutex<BTreeMap<String, Vec<Sub>>>>,
    pub tx: Sender<Command>,
    rx: Receiver<Command>,
}

impl Subs {
    pub fn new() -> Subs {
        let subs = Arc::new(Mutex::new(BTreeMap::<String, Vec<Sub>>::new()));

        let (tx, rx) = unbounded();

        Subs { subs, tx, rx }
    }

    pub fn handle(&self) {
        loop {
            let message = self.rx.recv().unwrap();

            match message {
                Command::New(sender, key, instr) => {
                    let last_time = Instant::now();

                    let mut subs = self.subs.lock().unwrap();
                    let senders = subs.entry(key).or_insert_with(Vec::new);

                    senders.push(Sub {
                        sender,
                        instr,
                        last_time,
                    });
                }

                Command::Call(key, val) => {
                    let mut subs = self.subs.lock().unwrap();

                    for alt_key in get_key_combinations(key.as_str()) {
                        let sub_vec = match subs.get_mut(&alt_key) {
                            Some(val) => val,

                            None => continue,
                        };

                        let mut bad_senders = Vec::<usize>::new();

                        for (i, sub) in sub_vec.iter_mut().enumerate() {
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

                            println!("Sending sub {}", msg);
                            if let Err(_) = sender.send(msg.into()) {
                                bad_senders.push(i);
                            } else {
                                sub.last_time = Instant::now();
                            }
                        }

                        for &i in bad_senders.iter().rev() {
                            sub_vec.swap_remove(i);
                        }

                        let count = bad_senders.len();
                        if count > 0 {
                            println!("Removing {} orphan subscriptions related to {}", count, key);
                        }
                    }
                }

                Command::Clean(secs) => {
                    // @todo

                    // let mut subs = self.subs.lock().unwrap();

                    // let mut count: u32 = 0;

                    // @todo We may need a way to test those connections.

                    // for (_, subs_vec) in subs.iter_mut() {
                    //     let mut orphans = Vec::<usize>::new();

                    //     for (i, sub) in subs_vec.iter().enumerate() {
                    //         if sub.last_time.elapsed().as_secs() > secs {
                    //             if let Err(_) = sub.sender.send(Result::Ping) {
                    //                 orphans.push(i);
                    //             }
                    //         }
                    //     }

                    //     for &i in orphans.iter().rev() {
                    //         subs_vec.swap_remove(i);
                    //         count += 1;
                    //     }
                    // }

                    // if count > 0 {
                    //     println!("Removing {} orphan subscriptions", count);
                    // }
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
