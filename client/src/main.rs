use std::{
    env,
    io::{self, BufRead, BufReader, BufWriter, Write},
    net::TcpStream,
};

fn main() {
    let mut server = "127.0.0.1:1984".to_owned();

    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        server = args[1].to_owned();
    }

    let stream = TcpStream::connect(server).unwrap();
    let stream_clone = stream.try_clone().unwrap();
    let mut writer = BufWriter::new(stream);
    let mut reader = BufReader::new(stream_clone);

    for line in io::stdin().lock().lines() {
        let content = line.unwrap();
        writer.write(content.as_bytes()).unwrap();
        writer.write(&[0xA]).unwrap();
        writer.flush().unwrap();

        let mut message = String::new();
        reader.read_line(&mut message).unwrap();

        match message.len() > 0 {
            true => println!("> {}\n", message.trim()),
            false => {
                println!("\n[!] Disconnected from the Server.\n");
                break;
            }
        }
    }
}

// Test server (maybe online) 142.93.180.20:1984
