use std::io::{self, Write};
use std::io::{BufRead, BufReader, BufWriter};
use std::net::TcpStream;

fn main() -> std::io::Result<()> {
    for line in io::stdin().lock().lines() {
        let stream = TcpStream::connect("127.0.0.1:1984")?;
        let stream_clone = stream.try_clone().unwrap();
        let mut writer = BufWriter::new(stream);
        let mut reader = BufReader::new(stream_clone);

        let content = line.unwrap();
        writer.write(content.as_bytes()).unwrap();
        writer.write(&[0xA]).unwrap();
        writer.flush().unwrap();

        let mut message = String::new();
        reader.read_line(&mut message).unwrap();
        println!("> {}\n", message.trim());
    }

    Ok(())
}
