use std::{
    collections::BTreeMap,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
};

use map::Result;

use crate::map;

pub enum Command {
    NewSub(Sender<map::Result>, String),
    CallSub(String),
}

pub struct Subs {
    pub subs: Arc<Mutex<BTreeMap<String, Vec<Sender<map::Result>>>>>,
    pub sender: Sender<Command>,
    receiver: Receiver<Command>,
}

impl Subs {
    pub fn new() -> Subs {
        let subs = Arc::new(Mutex::new(
            BTreeMap::<String, Vec<Sender<map::Result>>>::new(),
        ));

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
                Command::NewSub(sub_sender, key) => {
                    println!("NewSub!");

                    let mut subs = self.subs.lock().unwrap();

                    let senders = subs.entry(key).or_insert_with(Vec::new);

                    senders.push(sub_sender);
                }
                Command::CallSub(key) => {
                    println!("Found! callsub");

                    let mut subs = self.subs.lock().unwrap();
                    let key = key.clone();

                    let senders = subs.entry(key.clone()).or_insert_with(Vec::new);

                    // @todo This can be cached, by getting just one value and cloned it to others!

                    for sndr in senders {
                        let sndr_clone = sndr.clone();
                        map_sender
                            .send(map::Command::Json(sndr_clone, key.clone()))
                            .unwrap();
                    }
                }
            }
        }
    }
}
