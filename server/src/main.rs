mod map;
mod subs;
mod work;

mod db;
use db::DB;

mod parse;
use map::Result;
use parse::{AsyncInstr, Instr};
use work::ThreadPool;

use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
};

fn main() {
    println!("\nBIT:E");

    let listener = TcpListener::bind("0.0.0.0:1984").unwrap(); // Asumming Docker.
    let pool = Arc::new(Mutex::new(work::ThreadPool::new(4)));

    // Map & DB Thread.
    let map = map::Map::new();
    let map_sender = map.sender.clone();

    let mut db = DB::new(map.data.clone());
    db.load_from_file();

    let db_modified = db.modified.clone();
    let pool_clone = pool.clone();

    let mut pool = pool.lock().unwrap();
    pool.execute(move || map.handle(db_modified));
    pool.execute(move || db.handle(3));

    // Subscritions
    let subs = subs::Subs::new();
    let subs_sender = subs.sender.clone();

    let map_subs_sender = map_sender.clone();
    pool.execute(move || subs.handle(map_subs_sender));

    // New job on incoming connections.
    for stream in listener.incoming() {
        let pool_clone = pool_clone.clone();
        let stream = stream.unwrap();
        let map_sender = map_sender.clone();
        let subs_sender = subs_sender.clone();
        let (conn_sndr, conn_recvr) = mpsc::channel::<map::Result>();

        pool.execute(move || {
            handle_conn(
                stream,
                map_sender,
                subs_sender,
                conn_sndr,
                conn_recvr,
                pool_clone,
            )
        });
    }

    // @todo Thread waiting q! in the input to quit.
    println!("Shutting down.");
}

fn handle_conn(
    mut stream: TcpStream,
    map_sender: Sender<map::Command>,
    subs_sender: Sender<subs::Command>,
    conn_sndr: Sender<map::Result>,
    conn_recvr: Receiver<map::Result>,
    pool: Arc<Mutex<ThreadPool>>,
) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());

    loop {
        let mut buffer = String::new();

        if let Err(e) = reader.read_line(&mut buffer) {
            println!("Client disconnected: {}.", e);
            break;
        }

        if buffer.len() > 0 {
            println!("> {}", buffer.trim());
        } else {
            println!("Client disconnected.");
            break;
        }

        // Parse the message.
        let proc = parse::proc_from_string(buffer.as_str());
        let instr = proc.instr;
        let key = proc.key;
        let val = proc.value;

        let conn_sndr = conn_sndr.clone();

        let async_instr = match instr {
            Instr::Get => {
                if key.len() <= 0 {
                    AsyncInstr::No("OK".to_owned())
                } else {
                    map_sender.send(map::Command::Get(conn_sndr, key)).unwrap();

                    AsyncInstr::Yes
                }
            }
            Instr::Set => {
                if key.len() > 0 {
                    map_sender
                        .send(map::Command::Set(key.clone(), val))
                        .unwrap();

                    subs_sender.send(subs::Command::CallSub(key)).unwrap();
                }

                AsyncInstr::No(String::from("OK"))
            }
            Instr::Json => {
                map_sender.send(map::Command::Json(conn_sndr, key)).unwrap();

                AsyncInstr::Yes
            }
            Instr::Jtrim => {
                map_sender
                    .send(map::Command::Jtrim(conn_sndr, key))
                    .unwrap();

                AsyncInstr::Yes
            }
            Instr::SubJtrim => {
                println!("subjtrim");

                let stream = stream.try_clone().unwrap();
                let (sub_sndr, sub_rcvr) = mpsc::channel::<map::Result>();

                println!("subjtrim stream");

                let mut pool = pool.lock().unwrap();
                pool.execute(|| handle_sub(stream, sub_rcvr));

                println!("subjtrim pool");

                subs_sender
                    .send(subs::Command::NewSub(sub_sndr, key))
                    .unwrap();

                println!("subjtrim almost break");

                break;
            }
            Instr::Nop => AsyncInstr::No("NOP".to_owned()),
        };

        let message = match async_instr {
            AsyncInstr::Yes => match conn_recvr.recv().unwrap() {
                map::Result::Message(msg) => msg,
            },
            AsyncInstr::No(msg) => msg,
        };

        stream.write(message.as_bytes()).unwrap();
        stream.write(&[0xA]).unwrap(); // Write line.
        stream.flush().unwrap();
    }
}

fn handle_sub(mut stream: TcpStream, sub_recvr: Receiver<map::Result>) {
    loop {
        println!("Waiting for subs");

        let msg = match sub_recvr.recv().unwrap() {
            map::Result::Message(msg) => msg,
        };

        println!("{}", msg);

        stream.write(msg.as_bytes()).unwrap();
        stream.write(&[0xA]).unwrap(); // Write line.
        stream.flush().unwrap();
    }
}
