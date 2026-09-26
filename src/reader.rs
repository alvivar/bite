use std::{
    collections::HashMap,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
    time::Instant,
};

use crate::{
    cleaner,
    connection::Connection,
    parser::{self, Action::Parse},
};

use polling::{Event, Poller};

pub enum Action {
    Read(usize),
}

pub struct Reader {
    poller: Arc<Poller>,
    readers: Arc<Mutex<HashMap<usize, Connection>>>,
    pub tx: Sender<Action>,
    rx: Receiver<Action>,
}

impl Reader {
    pub fn new(poller: Arc<Poller>, readers: Arc<Mutex<HashMap<usize, Connection>>>) -> Reader {
        let (tx, rx) = channel::<Action>();

        Reader {
            poller,
            readers,
            tx,
            rx,
        }
    }

    pub fn handle(&self, parser_tx: Sender<parser::Action>, cleaner_tx: Sender<cleaner::Action>) {
        loop {
            match self.rx.recv().unwrap() {
                Action::Read(id) => {
                    let mut closed = false;

                    if let Some(connection) = self.readers.lock().unwrap().get_mut(&id) {
                        // The heartbeat may have already closed it and asked
                        // the cleaner to drop it.
                        if connection.closed {
                            continue;
                        }

                        read_messages(connection, &parser_tx);

                        if connection.closed {
                            closed = true;
                        } else {
                            self.poller
                                .modify(&connection.socket, Event::readable(id))
                                .unwrap();
                        }
                    }

                    if closed {
                        cleaner_tx.send(cleaner::Action::Drop(id)).unwrap();
                    }
                }
            }
        }
    }
}

/// Reads the available bytes and sends every complete message to the parser,
/// keeping any incomplete message buffered for the next read.
fn read_messages(connection: &mut Connection, parser_tx: &Sender<parser::Action>) {
    let id = connection.id;

    let data = match connection.try_read() {
        Ok(data) => data,

        Err(err) => {
            // connection.closed = true;
            // ^ This is already hapenning inside try_read() on errors.

            info!("Connection #{id} closed, read failed: {err}");

            return;
        }
    };

    if data.is_empty() {
        return;
    }

    connection.last_read = Instant::now();
    connection.messages.feed(&data);

    loop {
        match connection.messages.next_message() {
            Ok(Some(message)) if message.from != id as u32 => {
                connection.closed = true;

                let err = format!("message client id #{} is wrong", message.id);
                info!("Connection #{id} closed, bad message: {err}");

                return;
            }

            Ok(Some(message)) => parser_tx.send(Parse(message, connection.addr)).unwrap(),

            Ok(None) => return,

            Err(err) => {
                connection.closed = true;

                info!("Connection #{id} closed, bad message: {err}");

                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        net::{TcpListener, TcpStream},
        time::Duration,
    };

    #[test]
    fn incomplete_message_bytes_refresh_last_read() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (socket, addr) = listener.accept().unwrap();

        // The socket stays blocking so the read deterministically waits for
        // the bytes written below.
        let mut connection = Connection::new(1, socket, addr);
        connection.last_read = Instant::now() - Duration::from_secs(60);
        client.write_all(&[0, 1, 0]).unwrap();

        let (parser_tx, _parser_rx) = channel();
        read_messages(&mut connection, &parser_tx);

        assert!(connection.last_read.elapsed() < Duration::from_secs(30));
        assert!(connection.messages.is_incomplete());
        assert!(!connection.closed);
    }
}
