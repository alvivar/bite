# Bite

Minimal Multi-Threading Key-Value Store. Sockets waiting for messages to GET and
SET values.

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

The server runs on **127.0.0.1:1984**, just like me:

    cargo run --release --p server

The test client connects to **127.0.0.1:1984**.

    cargo run --release --p client

You should be able to write from any other TCP connection, just send complete
lines before flush.

## Tech

A Rust Multi-threading TcpListeners that stores data on a BTreeMap serialized
into a json file with Serde.
