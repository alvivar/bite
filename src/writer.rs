use crate::cleaner;
use crate::connection::Connection;
use crate::message::stamp_header;

use polling::{Event, Poller};

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

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
    writers: Arc<Mutex<HashMap<usize, Connection>>>,
    pub tx: Sender<Action>,
    rx: Receiver<Action>,
}

impl Writer {
    pub fn new(poller: Arc<Poller>, writers: Arc<Mutex<HashMap<usize, Connection>>>) -> Writer {
        let (tx, rx) = channel::<Action>();

        Writer {
            poller,
            writers,
            tx,
            rx,
        }
    }

    pub fn handle(&self, cleaner_tx: Sender<cleaner::Action>) {
        loop {
            match self.rx.recv().unwrap() {
                Action::Queue(order) => {
                    if let Some(connection) = self.writers.lock().unwrap().get_mut(&order.to_id) {
                        connection.send_queue.push(stamp_header(
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
                            connection.send_queue.push(stamp_header(
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
                        if !connection.send_queue.is_empty() {
                            let data = connection.send_queue.remove(0);

                            if let Err(err) = connection.try_write(data) {
                                println!("\nConnection #{id} broken, write failed: {err}");
                            }

                            connection.last_write = Instant::now();
                        }

                        if connection.closed {
                            closed = true;
                        } else if !connection.send_queue.is_empty() {
                            self.poll_write(connection);
                        } else {
                            self.poll_clean(connection);
                        }
                    }

                    if closed {
                        cleaner_tx.send(cleaner::Action::Drop(id)).unwrap();
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
