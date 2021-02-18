use std::{
    collections::BTreeMap,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
};

pub enum Command {
    Some,
}

pub enum Result {
    Wat,
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
                Command::Some => {}
            }
        }
    }
}
