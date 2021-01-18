use std::io::{BufWriter, Write};
use std::net::TcpStream;

fn main() -> std::io::Result<()> {
    let stream = TcpStream::connect("127.0.0.1:8888")?;
    let mut writer = BufWriter::new(stream);

    writer.write(b"      set          adros.eye0           Something about\n\nthe universe")?;
    writer.flush()?;

    // writer.write(b"s adros.eye1 Something about her")?;
    // writer.flush()?;

    // writer.write(b"g adros.eye1")?;
    // writer.flush()?;

    // writer.write(b"d adros.eye1")?;
    // writer.flush()?;

    // writer.buffer()

    // stream.write(&[0, 1, 0, 1])?;
    // stream.write(&[1, 1, 1, 1])?;

    // let mut data = [0 as u8; 8];
    // stream.read(&mut data)?;
    // println!("{:?}", &data);

    Ok(())
}

// id:2|op:1|type:1|val:?
