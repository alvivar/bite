use crate::connection::Connection;
use crate::message::stamp_header;
use crate::subs::{self, Action::DelAll};

use crossbeam_channel::{unbounded, Receiver, Sender};
use polling::{Event, Poller};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub enum Action {
    Queue(Order),
    QueueAll(Vec<Order>),
    Write(usize),
}

pub struct Order {
    pub from_id: usize,
    pub to_id: usize,
    pub msg_id: usize,
    pub data: Vec<u8>,
}

pub struct Writer {
    poller: Arc<Poller>,
    readers: Arc<Mutex<HashMap<usize, Connection>>>,
    writers: Arc<Mutex<HashMap<usize, Connection>>>,
    lost: Arc<Mutex<Vec<usize>>>,
    pub tx: Sender<Action>,
    rx: Receiver<Action>,
}

impl Writer {
    pub fn new(
        poller: Arc<Poller>,
        readers: Arc<Mutex<HashMap<usize, Connection>>>,
        writers: Arc<Mutex<HashMap<usize, Connection>>>,
        lost: Arc<Mutex<Vec<usize>>>,
    ) -> Writer {
        let (tx, rx) = unbounded::<Action>();

        Writer {
            poller,
            writers,
            readers,
            lost,
            tx,
            rx,
        }
    }

    pub fn handle(&self, subs_tx: Sender<subs::Action>) {
        loop {
            match self.rx.recv().unwrap() {
                Action::Queue(order) => {
                    if let Some(connection) = self.writers.lock().unwrap().get_mut(&order.to_id) {
                        connection.to_send.push(stamp_header(
                            order.data,
                            order.from_id as u32,
                            order.msg_id as u32,
                        ));

                        self.poll_write(connection);
                    }
                }

                Action::QueueAll(orders) => {
                    let mut writers = self.writers.lock().unwrap();
                    for order in orders {
                        if let Some(connection) = writers.get_mut(&order.to_id) {
                            connection.to_send.push(stamp_header(
                                order.data,
                                order.from_id as u32,
                                order.msg_id as u32,
                            ));

                            self.poll_write(connection);
                        }
                    }
                }

                Action::Write(id) => {
                    let mut closed = false;
                    if let Some(connection) = self.writers.lock().unwrap().get_mut(&id) {
                        if !connection.to_send.is_empty() {
                            let data = connection.to_send.remove(0);

                            if let Err(err) = connection.try_write(data) {
                                println!("\nConnection #{} broken, write failed: {}", id, err);
                            }
                        }

                        if connection.closed {
                            closed = true;
                        } else if !connection.to_send.is_empty() {
                            self.poll_write(connection);
                        } else {
                            self.poll_clean(connection);
                        }
                    }

                    if closed {
                        let readers = self.readers.lock().unwrap().remove(&id).unwrap();
                        let writers = self.writers.lock().unwrap().remove(&id).unwrap();
                        self.poller.delete(&readers.socket).unwrap();
                        self.poller.delete(&writers.socket).unwrap();
                        self.lost.lock().unwrap().push(id);
                        subs_tx.send(DelAll(id)).unwrap();
                    }
                }
            }
        }
    }

    fn poll_write(&self, connection: &mut Connection) {
        self.poller
            .modify(&connection.socket, Event::writable(connection.id))
            .unwrap();
    }

    fn poll_clean(&self, connection: &mut Connection) {
        self.poller
            .modify(&connection.socket, Event::none(connection.id))
            .unwrap();
    }
}
