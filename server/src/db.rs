use serde_json;

use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{Read, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::sleep,
    time::Duration,
};

const DB_PATH: &str = "./data";
const DB_FILE: &str = "./data/DB.json";

pub struct DB {
    data: Arc<Mutex<BTreeMap<String, String>>>,
    pub modified: Arc<AtomicBool>,
}

impl DB {
    pub fn new(data: Arc<Mutex<BTreeMap<String, String>>>) -> DB {
        let modified = Arc::new(AtomicBool::new(false));

        DB { data, modified }
    }

    pub fn handle(&mut self, throttle: u64) {
        loop {
            sleep(Duration::new(throttle, 0));

            if self.modified.load(Ordering::Relaxed) {
                self.save_to_file();

                self.modified.swap(false, Ordering::Relaxed);

                println!("DB saved.");
            }
        }
    }

    pub fn load_from_file(&self) {
        fs::create_dir_all(DB_PATH).unwrap();

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

    pub fn save_to_file(&self) {
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
