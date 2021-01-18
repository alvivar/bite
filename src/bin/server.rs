use std::collections::{btree_map::Values, BTreeMap};
use std::io::{BufRead, BufReader, Read};
use std::net::{TcpListener, TcpStream};

fn main() -> std::io::Result<()> {
    let mut key_values: BTreeMap<&str, &str> = BTreeMap::new();
    let listener = TcpListener::bind("127.0.0.1:8888")?;
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let res = parse_instruction(stream);
                println!("{:?}", res);
            }
            Err(e) => {
                println!("Somehow an error:\n{}\n", e);
            }
        }
    }

    Ok(())
}

fn parse_instruction(mut stream: TcpStream) -> std::io::Result<(String, String, String)> {
    let mut reader = BufReader::new(&mut stream);
    let mut content = String::new();
    reader.read_to_string(&mut content)?;

    let mut instruction = String::new();
    let mut key = String::new();
    let mut value = String::new();

    let mut found = 0;
    for c in content.chars() {
        match c {
            ' ' => {
                if value.len() > 0 {
                    value.push(' ');
                } else if key.len() > 0 {
                    found = 2;
                } else if instruction.len() > 0 {
                    found = 1;
                }
            }
            _ => match found {
                0 => {
                    instruction.push(c);
                }
                1 => {
                    key.push(c);
                }
                _ => {
                    value.push(c);
                }
            },
        }
    }

    // let mut parsed = content.trim().splitn(3, ' ');
    // let instruction = parsed.next().unwrap();
    // let key = parsed.next().unwrap();
    // let value: String = parsed.collect();

    Ok((instruction.to_string(), key.to_string(), value))
}
