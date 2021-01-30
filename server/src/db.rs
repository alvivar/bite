use serde_json;

use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::{Read, Write},
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread::sleep,
    time::{Duration, Instant},
    u64,
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
    save_throttle: u64,
    saving: bool,
    timer: Instant,
}

impl DB {
    pub fn new(data: Arc<Mutex<BTreeMap<String, String>>>, save_throttle: u64) -> DB {
        let (sender, receiver) = mpsc::channel();

        DB {
            data,
            sender,
            receiver,
            save_throttle,
            timer: Instant::now(),
            saving: false,
        }
    }

    pub fn handle(&mut self) {
        loop {
            let message = self.receiver.try_recv();
            match message {
                Ok(m) => match m {
                    Command::Load => self.load_from_file(),
                    Command::Save => {
                        self.saving = true;
                        self.timer = Instant::now();
                    }
                },
                Err(_) => {
                    if self.saving && self.timer.elapsed().as_secs() >= self.save_throttle {
                        self.saving = false;
                        self.save_to_file();
                    } else {
                        sleep(Duration::new(0, 100000000)); // 0.1s
                    }
                }
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
