use crossbeam_channel::{unbounded, Receiver, Sender};

use std::{
    collections::BTreeMap,
    io::Write,
    net::TcpStream,
    sync::{Arc, Mutex},
    time::Instant,
    u64,
};

pub enum Command {
    New(String, TcpStream),
    Touch(String),
    Clean(u64),
}

struct Conn {
    stream: TcpStream,
    last_time: Instant,
}

impl Conn {
    fn new(stream: TcpStream) -> Conn {
        let last_time = Instant::now();

        Conn { stream, last_time }
    }
}

pub struct Heartbeat {
    conns: Arc<Mutex<BTreeMap<String, Conn>>>,
    pub sender: Sender<Command>,
    receiver: Receiver<Command>,
}

impl Heartbeat {
    pub fn new() -> Heartbeat {
        let conns = Arc::new(Mutex::new(BTreeMap::<String, Conn>::new()));
        let (sender, receiver) = unbounded();

        Heartbeat {
            conns,
            sender,
            receiver,
        }
    }

    pub fn handle(&self) {
        loop {
            let message = self.receiver.recv().unwrap();

            match message {
                Command::New(addr, stream) => {
                    let mut conns = self.conns.lock().unwrap();

                    conns.entry(addr).or_insert(Conn::new(stream));
                }

                Command::Touch(addr) => {
                    let mut conns = self.conns.lock().unwrap();

                    if let Some(val) = conns.get_mut(&addr) {
                        val.last_time = Instant::now();
                    }
                }

                Command::Clean(secs) => {
                    let mut conns = self.conns.lock().unwrap();

                    let mut orphans = Vec::<String>::new();

                    for (addr, conn) in conns.iter() {
                        if conn.last_time.elapsed().as_secs() > secs {
                            if let Err(e) = stream_write(&conn.stream, "?") {
                                orphans.push(addr.to_owned());

                                println!("Hearbeat to {} failed: {}", addr, e);
                            }
                        }
                    }

                    for key in orphans.iter() {
                        conns.remove(key);
                    }
                }
            }
        }
    }
}

fn stream_write(mut stream: &TcpStream, message: &str) -> std::io::Result<()> {
    stream.write(message.as_bytes())?;
    stream.write(&[0xA])?; // Write line.
    stream.flush()?;

    Ok(())
}
