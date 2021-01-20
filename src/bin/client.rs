use std::io::{self, Write};
use std::io::{BufRead, BufWriter};
use std::net::TcpStream;

fn main() -> std::io::Result<()> {
    let stdin = io::stdin();
    let stream = TcpStream::connect("127.0.0.1:1984")?;
    let mut writer = BufWriter::new(stream);

    for line in stdin.lock().lines() {
        let content = line.unwrap();
        writer.write(content.as_bytes()).unwrap();
        writer.flush().unwrap();
        break;
    }

    Ok(())
}
