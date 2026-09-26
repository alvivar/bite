use std::{
    collections::HashMap,
    sync::mpsc::{channel, Receiver, Sender},
};

use crate::{
    parser::Command,
    writer::{self, Action::QueueAll, Order},
};

use serde_json::json;

pub enum Action {
    Add(String, usize, Command),
    Del(String, usize),
    DelAll(usize),
    Call(String, Vec<u8>, usize, usize),
}

pub struct Sub {
    id: usize,
    command: Command,
}

pub struct Subs {
    key_subs: HashMap<String, Vec<Sub>>,
    id_keys: HashMap<usize, Vec<String>>,
    pub tx: Sender<Action>,
    rx: Receiver<Action>,
}

impl Subs {
    pub fn new() -> Subs {
        let key_subs = HashMap::<String, Vec<Sub>>::new();
        let id_keys = HashMap::<usize, Vec<String>>::new();
        let (tx, rx) = channel::<Action>();

        Subs {
            key_subs,
            id_keys,
            tx,
            rx,
        }
    }

    pub fn handle(&mut self, writer_tx: Sender<writer::Action>) {
        loop {
            match self.rx.recv().unwrap() {
                Action::Add(key, id, command) => self.subscribe(key, id, command),

                Action::Del(key, id) => self.unsubscribe(&key, id),

                Action::DelAll(id) => self.unsubscribe_all(id),

                Action::Call(key, data, from_id, msg_id) => {
                    let mut messages = Vec::<Order>::new();

                    for alt_key in get_key_combinations(key.as_str()) {
                        if let Some(subs) = self.key_subs.get(&alt_key) {
                            for sub in subs {
                                let data = match sub.command {
                                    Command::SubGet => data.to_owned(),

                                    Command::SubKeyValue => {
                                        let key = key.split('.').last().unwrap();
                                        let mut message = Vec::<u8>::new();

                                        message.extend(key.as_bytes());
                                        message.extend(" ".as_bytes());
                                        message.extend(&data);
                                        message
                                    }

                                    Command::SubJson => {
                                        let key = key.split('.').last().unwrap();
                                        let message = String::from_utf8_lossy(&data);
                                        json!({ key: message }).to_string().into_bytes()
                                    }

                                    _ => unreachable!(),
                                };

                                messages.push(Order {
                                    from_id,
                                    to_id: sub.id,
                                    msg_id,
                                    data,
                                });
                            }
                        }
                    }

                    if !messages.is_empty() {
                        writer_tx.send(QueueAll(messages)).unwrap();
                    }
                }
            }
        }
    }

    fn subscribe(&mut self, key: String, id: usize, command: Command) {
        let keys = self.id_keys.entry(id).or_default();

        if !keys.contains(&key) {
            keys.push(key.to_owned());
        }

        let subs = self.key_subs.entry(key).or_default();

        if !subs.iter().any(|x| x.id == id && x.command == command) {
            subs.push(Sub { id, command })
        }
    }

    fn unsubscribe(&mut self, key: &str, id: usize) {
        self.remove_subs(key, id);

        if let Some(keys) = self.id_keys.get_mut(&id) {
            keys.retain(|k| k != key);

            if keys.is_empty() {
                self.id_keys.remove(&id);
            }
        }
    }

    fn unsubscribe_all(&mut self, id: usize) {
        if let Some(keys) = self.id_keys.remove(&id) {
            for key in keys {
                self.remove_subs(&key, id);
            }
        }
    }

    /// Removes every subscription of the client to the key, and the key itself
    /// when nobody else is subscribed.
    fn remove_subs(&mut self, key: &str, id: usize) {
        if let Some(subs) = self.key_subs.get_mut(key) {
            subs.retain(|x| x.id != id);

            if subs.is_empty() {
                self.key_subs.remove(key);
            }
        }
    }
}

/// "data.inner.value" -> ["data.inner.value", "data.inner", "data"]
fn get_key_combinations(key: &str) -> Vec<String> {
    let mut parent_keys = Vec::<String>::new();

    let keys: Vec<&str> = key.split('.').collect();
    let len = keys.len();

    for i in 0..len {
        let end = len - i;
        let str = keys[..end].join(".");
        parent_keys.push(str);
    }

    parent_keys
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subscriber_ids(subs: &Subs, key: &str) -> Vec<usize> {
        subs.key_subs[key].iter().map(|x| x.id).collect()
    }

    fn subscribed_keys(subs: &Subs) -> Vec<&str> {
        let mut keys: Vec<&str> = subs.key_subs.keys().map(String::as_str).collect();
        keys.sort();
        keys
    }

    #[test]
    fn unsubscribing_unknown_key_or_client_changes_nothing() {
        let mut subs = Subs::new();
        subs.subscribe("a".into(), 1, Command::SubGet);

        subs.unsubscribe("missing", 1);
        subs.unsubscribe("a", 2);
        subs.unsubscribe("a", 2);

        assert_eq!(subscribed_keys(&subs), ["a"]);
        assert_eq!(subscriber_ids(&subs, "a"), [1]);
        assert_eq!(subs.id_keys, HashMap::from([(1, vec!["a".into()])]));
    }

    #[test]
    fn unsubscribe_removes_every_format_of_the_client_only() {
        let mut subs = Subs::new();
        subs.subscribe("a".into(), 1, Command::SubGet);
        subs.subscribe("a".into(), 1, Command::SubKeyValue);
        subs.subscribe("a".into(), 1, Command::SubJson);
        subs.subscribe("a".into(), 2, Command::SubGet);
        subs.subscribe("b".into(), 1, Command::SubGet);

        subs.unsubscribe("a", 1);

        assert_eq!(subscriber_ids(&subs, "a"), [2]);
        assert_eq!(subscriber_ids(&subs, "b"), [1]);
        assert_eq!(
            subs.id_keys,
            HashMap::from([(1, vec!["b".into()]), (2, vec!["a".into()])])
        );
    }

    #[test]
    fn unsubscribing_last_key_removes_key_and_client() {
        let mut subs = Subs::new();
        subs.subscribe("a".into(), 1, Command::SubGet);

        subs.unsubscribe("a", 1);

        assert!(subs.key_subs.is_empty());
        assert!(subs.id_keys.is_empty());
    }

    #[test]
    fn disconnect_after_unsubscribe_cleans_remaining_keys() {
        let mut subs = Subs::new();
        subs.subscribe("a".into(), 1, Command::SubGet);
        subs.subscribe("b".into(), 1, Command::SubGet);
        subs.subscribe("b".into(), 1, Command::SubJson);
        subs.subscribe("c".into(), 1, Command::SubGet);
        subs.subscribe("b".into(), 2, Command::SubGet);

        subs.unsubscribe("a", 1);
        subs.unsubscribe_all(1);

        assert_eq!(subscribed_keys(&subs), ["b"]);
        assert_eq!(subscriber_ids(&subs, "b"), [2]);
        assert_eq!(subs.id_keys, HashMap::from([(2, vec!["b".into()])]));
    }
}
