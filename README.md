# Bite

Minimal Key-Value Store. A socket waiting for messages to GET and SET values.

## How to use

To set the value:

    SET somekeyname Some string as a value I guess
    > OK

To get the value:

    GET somekeyname
    > Some string as a value I guess

    GET keywithoutvalue
    >

A **./data/DB.json** file will be created with the information sorted.

## How to run

The server runs on **127.0.0.1:1984**, just like me:

    cargo run --release --p server

The client is just a simple test that connects to **127.0.0.1:1984** and
send/receive messages.

    cargo run --release --p client

Or for a custom test server:

    cargo run --release && ./target/release/client 123.45.678.90:1234

You should be able to write from any TCP connection, just send complete lines
before flush.

## Docker installation

It includes the **docker-compose** and **Dockerfile** to build and run the
server.

    docker-compose build
    docker-compose up -d

Then you can connect at **127.0.0.1:1984**.

## Tech

Rust multi-thread **TcpListener** storing data on a **BTreeMap** serialized into
a json file with **Serde**. Ready to run on **Docker**.

## Things to do

- Auth
- Lists (Push, Pop)
-
