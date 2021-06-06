use std::{
    thread::{self, sleep},
    time::Duration,
};

use tungstenite::{client::AutoStream, connect, http::Response, Message, WebSocket};
use url::Url;

struct Messenger {
    msg_socket: WebSocket<AutoStream>,
    msg_response: Response<()>,
    sub_socket: WebSocket<AutoStream>,
    sub_response: Response<()>,
}

impl Messenger {
    fn new(url: String) -> Messenger {
        let (msg_socket, msg_response) = connect(Url::parse(&url).unwrap()).unwrap();
        let (sub_socket, sub_response) = connect(Url::parse(&url).unwrap()).unwrap();

        println!("\nConnected to the server!");
        println!("Msg channel response HTTP code: {}", msg_response.status());
        println!("Sub channel response HTTP code: {}", sub_response.status());

        Messenger {
            msg_socket,
            msg_response,
            sub_socket,
            sub_response,
        }
    }

    fn print_msg_headers(&self) {
        println!("\nMsg response contains the following headers:");
        for (ref header, ref value) in self.msg_response.headers() {
            println!("* {}: {:?}", header, value);
        }
    }

    fn print_sub_headers(&self) {
        println!("\nSub response contains the following headers:");
        for (ref header, ref value) in self.sub_response.headers() {
            println!("* {}: {:?}", header, value);
        }
    }

    fn handle_msg(&mut self) {
        loop {
            // Block reading a channel to send a message.
            self.msg_socket
                .write_message(Message::Text("Hello WebSocket".into()))
                .unwrap();

            let msg = self.msg_socket.read_message().unwrap();

            sleep(Duration::new(3, 0));

            println!("Received: {}", msg);
        }
    }

    fn handle_sub(&mut self) {
        loop {
            // Just receive.
            let sub = self.sub_socket.read_message().unwrap();

            println!("Received: {}", sub);
        }
    }
}

fn main() {
    env_logger::init();

    let mut messenger = Messenger::new("ws://localhost:1984/socket".into());
    messenger.print_msg_headers();
    messenger.print_sub_headers();

    thread::spawn(move || messenger.handle_msg());
}
