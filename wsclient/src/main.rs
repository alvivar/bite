use std::thread::{self};

use tungstenite::{client::AutoStream, connect, http::Response, Message, WebSocket};
use url::Url;

struct Messenger {
    socket: WebSocket<AutoStream>,
    response: Response<()>,
}

impl Messenger {
    fn new(url: String) -> Messenger {
        let (socket, response) = connect(Url::parse(&url).unwrap()).unwrap();

        println!("\nConnected to the server!");
        println!("Response HTTP code: {}", response.status());

        Messenger { socket, response }
    }

    fn print_headers(&self) {
        println!("\nResponse contains the following headers:");
        for (ref header, ref value) in self.response.headers() {
            println!("* {}: {:?}", header, value);
        }
    }

    fn handle_msg(&mut self) {
        loop {
            self.socket
                .write_message(Message::Text("Hello WebSocket".into()))
                .unwrap();

            let msg = self.socket.read_message().unwrap();

            println!("Received: {}", msg);
        }
    }

    fn handle_sub(&mut self) {
        loop {
            let sub = self.socket.read_message().unwrap();

            println!("Received: {}", sub);
        }
    }
}

fn main() {
    env_logger::init();

    let mut msg = Messenger::new("ws://localhost:1984/socket".into());
    msg.print_headers();

    thread::spawn(move || msg.handle_msg());

    // let mut sub = Messenger::new("ws://localhost:1984/socket".into());
    // sub.print_headers();
    // thread::spawn(move || sub.handle_sub());
}
