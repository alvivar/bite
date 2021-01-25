use std::io::{self, Write};
use std::io::{BufRead, BufReader, BufWriter};
use std::net::TcpStream;

fn main() -> std::io::Result<()> {
    let stream = TcpStream::connect("127.0.0.1:1984")?;
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

        if message.len() > 0 {
            println!("> {}\n", message.trim());
        } else {
            println!("\n[!] Disconnected from the Server.\n");
            break;
        }
    }

    Ok(())
}
