use std::{
    collections::HashMap,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, RwLock,
    },
    time::Instant,
};

use crate::{cleaner, connection::Connection, message::stamp_header};

use polling::{Event, Poller};

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
    writers: Arc<RwLock<HashMap<usize, Connection>>>,
    pub tx: Sender<Action>,
    rx: Receiver<Action>,
}

impl Writer {
    pub fn new(poller: Arc<Poller>, writers: Arc<RwLock<HashMap<usize, Connection>>>) -> Writer {
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
                    if let Some(connection) = self.writers.write().unwrap().get_mut(&order.to_id) {
                        connection.send_queue.push(stamp_header(
                            order.data,
                            order.from_id as u32,
                            order.msg_id as u32,
                        ));

                        self.poll_writable(connection);
                    }
                }

                Action::QueueAll(orders) => {
                    let mut writers = self.writers.write().unwrap();
                    for order in orders {
                        if let Some(connection) = writers.get_mut(&order.to_id) {
                            connection.send_queue.push(stamp_header(
                                order.data,
                                order.from_id as u32,
                                order.msg_id as u32,
                            ));

                            self.poll_writable(connection);
                        }
                    }
                }

                Action::Write(id) => {
                    let mut closed = false;

                    if let Some(connection) = self.writers.write().unwrap().get_mut(&id) {
                        if !connection.send_queue.is_empty() {
                            let data = connection.send_queue.remove(0);

                            if let Err(err) = connection.try_write(data) {
                                info!("Connection #{id} broken, write failed: {err}");
                            }

                            connection.last_write = Instant::now();
                        }

                        if connection.closed {
                            closed = true;
                        } else if !connection.send_queue.is_empty() {
                            self.poll_writable(connection);
                        } else {
                            self.poll_none(connection);
                        }
                    }

                    if closed {
                        cleaner_tx.send(cleaner::Action::Drop(id)).unwrap();
                    }
                }
            }
        }
    }

    fn poll_writable(&self, connection: &mut Connection) {
        self.poller
            .modify(&connection.socket, Event::writable(connection.id))
            .unwrap();
    }

    fn poll_none(&self, connection: &mut Connection) {
        self.poller
            .modify(&connection.socket, Event::none(connection.id))
            .unwrap();
    }
}
