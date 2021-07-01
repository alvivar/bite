use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crossbeam_channel::{unbounded, Receiver, Sender};

use crate::{conn::Connection, Work};

pub enum Cmd {
    Add(String, usize),
    Del(String, usize),
    Call(String, String),
}

pub struct Subs {
    registry: HashMap<String, Vec<usize>>,
    write_map: Arc<Mutex<HashMap<usize, Connection>>>,
    work_tx: Sender<Work>,
    pub tx: Sender<Cmd>,
    rx: Receiver<Cmd>,
}

impl Subs {
    pub fn new(write_map: Arc<Mutex<HashMap<usize, Connection>>>, work_tx: Sender<Work>) -> Subs {
        let registry = HashMap::<String, Vec<usize>>::new();
        let (tx, rx) = unbounded::<Cmd>();

        Subs {
            registry,
            write_map,
            work_tx,
            tx,
            rx,
        }
    }

    pub fn handle(&mut self) {
        loop {
            match self.rx.recv() {
                Ok(Cmd::Add(key, id)) => {
                    let subs = self.registry.entry(key).or_insert_with(Vec::new);

                    if subs.iter().any(|x| x == &id) {
                        continue;
                    }

                    subs.push(id)
                }

                Ok(Cmd::Del(key, id)) => {
                    let subs = self.registry.entry(key).or_insert_with(Vec::new);
                    subs.retain(|x| x != &id);
                }

                Ok(Cmd::Call(key, value)) => {
                    if let Some(subs) = self.registry.get(&key) {
                        let mut write_map = self.write_map.lock().unwrap();
                        for id in subs {
                            if let Some(conn) = write_map.remove(id) {
                                self.work_tx
                                    .send(Work::Write(conn, key.to_owned(), value.to_owned()))
                                    .unwrap();
                            }
                        }
                    }
                }

                Err(err) => panic!("The subs channel failed: {}", err),
            }
        }
    }
}
