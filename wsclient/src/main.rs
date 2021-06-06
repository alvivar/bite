use std::{thread::sleep, time::Duration};

use tungstenite::{connect, Message};
use url::Url;

fn main() {
    env_logger::init();

    let (mut socket, response) =
        connect(Url::parse("ws://localhost:1984/socket").unwrap()).expect("Can't connect");

    println!("Connected to the server");
    println!("Response HTTP code: {}", response.status());
    println!("Response contains the following headers:");

    for (ref header, _value) in response.headers() {
        println!("* {}", header);
    }

    loop {
        socket
            .write_message(Message::Text("Hello WebSocket".into()))
            .unwrap();

        let msg = socket.read_message().expect("Error reading message");

        sleep(Duration::new(3, 0));

        println!("Received: {}", msg);
    }
    // socket.close(None);
}
