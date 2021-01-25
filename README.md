# Bite

Minimalistic Key-Value Store. A socket waiting for messages to GET and SET
values.

## How to use

To set the value:

    SET somekeyname Some string as a value I guess
    > OK

To get the value:

    GET somekeyname
    > Some string as a value I guess

    GET keywithoutvalue
    >

## How to run

Runs on **127.0.0.1:1984**, just like me:

    cargo run --release --bin server

A simple test client that connects to **127.0.0.1:1984**. But you should be able
to write from any TCP connection, just write complete lines before flush.

    cargo run --release --bin client

## Tech

A Rust TcpListener that stores data on a BTreeMap serialized into a json file
with Serde.
