use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

const DB_PATH: &str = "./data";
const DB_FILE: &str = "./data/db.bin";

pub struct DB {
    data: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
    pub modified: Arc<AtomicBool>,
}

impl DB {
    pub fn new(data: Arc<Mutex<BTreeMap<String, Vec<u8>>>>) -> DB {
        let modified = Arc::new(AtomicBool::new(false));

        DB { data, modified }
    }

    pub fn handle(&mut self, throttle: u64) {
        loop {
            sleep(Duration::new(throttle, 0));

            if self.modified.swap(false, Ordering::Relaxed) {
                self.save_to_file();
            }
        }
    }

    pub fn load_from_file(&self) {
        fs::create_dir_all(DB_PATH).unwrap();

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(DB_FILE);

        let mut content = Vec::<u8>::new();
        file.unwrap().read_to_end(&mut content).unwrap();

        if content.is_empty() {
            return;
        }

        if let Ok(data) = bincode::deserialize::<BTreeMap<String, Vec<u8>>>(&content[..]) {
            let mut map = self.data.lock().unwrap();
            *map = data;
        }
    }

    pub fn save_to_file(&self) {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(DB_FILE);

        let map = self.data.lock().unwrap();
        let data: Vec<u8> = bincode::serialize(&*map).unwrap();
        file.unwrap().write_all(&data[..]).unwrap();

        let filename = DB_FILE.split('/').last().unwrap();
        println!("\n{filename} saved");
    }
}
