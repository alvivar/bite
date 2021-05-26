use tungstenite::{connect, Message};
use url::Url;

use std::{env, io};

fn main() {
    let mut server = "ws://localhost:1984/socket".to_owned();

    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        server = args[1].to_owned();
    }

    let (mut socket, _) = connect(Url::parse(&server).unwrap()).unwrap();

    let mut sub_mode = false;
    let mut input = String::new();

    loop {
        if !sub_mode {
            input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            socket
                .write_message(Message::Text(input.trim().to_string()))
                .unwrap();
        }

        if !sub_mode && input.trim().starts_with("#") {
            sub_mode = true;
        }

        let message = socket.read_message().unwrap();
        println!("read_message(), {}", message);

        match message.len() > 0 {
            true => println!("> {}\n", message),
            false => {
                println!("\n[!] Disconnected from the Server.\n");
                break;
            }
        }
    }
}

// Test server (maybe online) 142.93.180.20:1984
