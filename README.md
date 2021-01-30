# Bite

Minimalistic Key-Value Store. Multi-threading sockets waiting for messages to GET and SET
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

A **DB.json** file will be created with the information sorted.

## How to run

Runs on **127.0.0.1:1984**, just like me:

    cargo run --release --p server

A simple test client that connects to **127.0.0.1:1984**. But you should be able
to write from any other TCP connection, just write complete lines before flush.

    cargo run --release --p client

## Tech

Multi-threading TcpListeners that stores data on a BTreeMap serialized into a json file
with Serde. Made. in. Rust.
