use serde_json;

use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::{Read, Write},
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
};

const DB_FILE: &str = "DB.json";

pub enum Command {
    Load,
    Save,
}

pub struct DB {
    data: Arc<Mutex<BTreeMap<String, String>>>,
    pub sender: Sender<Command>,
    receiver: Receiver<Command>,
}

impl DB {
    pub fn new(data: Arc<Mutex<BTreeMap<String, String>>>) -> DB {
        let (sender, receiver) = mpsc::channel();

        DB {
            data,
            sender,
            receiver,
        }
    }

    pub fn handle(&self) {
        loop {
            let message = self.receiver.recv().unwrap();

            match message {
                Command::Load => self.load_from_file(),
                Command::Save => self.save_to_file(),
            }
        }
    }

    fn load_from_file(&self) {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(DB_FILE)
            .unwrap();

        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        if contents.len() > 0 {
            let c = self.data.clone();
            let mut map = c.lock().unwrap();
            *map = serde_json::from_str(&contents).unwrap();
        }
    }

    fn save_to_file(&self) {
        let map = self.data.lock().unwrap();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(DB_FILE);

        let json = serde_json::to_string(&*map).unwrap();
        file.unwrap().write_all(json.as_bytes()).unwrap();
    }
}
