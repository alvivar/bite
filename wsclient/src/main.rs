use std::{thread::sleep, time::Duration};

use tungstenite::{client::AutoStream, connect, http::Response, Message, WebSocket};
use url::Url;

struct Messenger {
    socket: WebSocket<AutoStream>,
    response: Response<()>,
}

impl Messenger {
    fn new(url: String) -> Messenger {
        let (socket, response) = connect(Url::parse(&url).unwrap()).unwrap();

        Messenger { socket, response }
    }

    fn connect(&mut self) {
        println!("Connected to the server");
        println!("Response HTTP code: {}", self.response.status());

        println!("Response contains the following headers:");
        for (ref header, ref value) in self.response.headers() {
            println!("* {}: {:?}", header, value);
        }
    }

    fn handle_msg(&mut self) {
        loop {
            self.socket
                .write_message(Message::Text("Hello WebSocket".into()))
                .unwrap();

            let msg = self.socket.read_message().expect("Error reading message");

            sleep(Duration::new(3, 0));

            println!("Received: {}", msg);
        }
    }
}

fn main() {
    env_logger::init();

    let mut messenger = Messenger::new("ws://localhost:1984/socket".into());
    messenger.connect();
    messenger.handle_msg();
}
