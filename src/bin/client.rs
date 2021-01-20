use std::io::{BufWriter, Write};
use std::net::TcpStream;

fn main() -> std::io::Result<()> {
    let stream = TcpStream::connect("127.0.0.1:8888")?;
    let mut writer = BufWriter::new(stream);

    writer.write(b"      set          adros.eye0           baraka")?;
    writer.flush()?;

    Ok(())
}
