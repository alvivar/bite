use std::{
    collections::BTreeMap,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
};

pub enum Command {
    Sub,
    Check,
}

pub enum Result {
    Message(String),
}

pub struct Subs {
    pub subs: Arc<Mutex<BTreeMap<String, Vec<Sender<Result>>>>>,
    pub sender: Sender<Command>,
    receiver: Receiver<Command>,
}

impl Subs {
    pub fn new() -> Subs {
        let subs = Arc::new(Mutex::new(BTreeMap::<String, Vec<Sender<Result>>>::new()));

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
                Command::Sub => {
                    // Subscribes the client thread
                }
                Command::Check => {
                    // Checks the key and sends the each client
                }
            }
        }
    }
}
