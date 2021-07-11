use std::{
    collections::HashMap,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
};

use polling::{Event, Poller};

use crate::conn::Connection;

pub enum Cmd {
    Add(String, usize),
    Del(String, usize),
    Call(String, String),
}

pub struct Subs {
    registry: HashMap<String, Vec<usize>>,
    writers: Arc<Mutex<HashMap<usize, Connection>>>,
    poller: Arc<Poller>,
    pub tx: Sender<Cmd>,
    rx: Receiver<Cmd>,
}

impl Subs {
    pub fn new(writers: Arc<Mutex<HashMap<usize, Connection>>>, poller: Arc<Poller>) -> Subs {
        let registry = HashMap::<String, Vec<usize>>::new();
        let (tx, rx) = channel::<Cmd>();

        Subs {
            registry,
            writers,
            poller,
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
                        let mut writers = self.writers.lock().unwrap();
                        for id in subs {
                            if let Some(conn) = writers.get_mut(id) {
                                conn.data = format!("{} {}", key, value).into();
                                self.poller
                                    .modify(&conn.socket, Event::writable(conn.id))
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
