# BITE

Key-Value database with subscriptions, aimed to real time multiplayer
applications.

    _...DEMO in progress_

## C# Library

Check out [**.csharp**](https://github.com/alvivar/bite/tree/master/.csharp) for
a simple **C#** client library and a **Unity** example.

## Docker

It includes the **docker-compose** and **Dockerfile** to build and run the
server.

    docker-compose build
    docker-compose up -d

Ready to run on **127.0.0.1:1984**.

## Rust

The server runs on **0.0.0.0:1984**, you can send/receive messages with any TCP
connection, just send complete lines (0xA). @todo Check this.

    cargo run --release --p server

The client is a simple test that connects to **127.0.0.1:1984**, write and hit
enter to send/receive.

    cargo run --release --p client

You could use the first argument to specify a different address.

    cargo run --release -p client -- 123.45.678.90:1234

## Tech

**Rust** multi-thread **TcpListeners**, using **polling** from **smol** to
handle sockets events, storing on a **BTreeMap** serialized into a json file
with **Serde**.

Uses [Google Container
Tools](https://github.com/GoogleContainerTools/distroless/blob/master/examples/rust/Dockerfile)
to run the binary on **Docker**.

## Things that I would like to add

-   Auth (Soon, working on a Tokio/Warp proxy with security)
-   The BTree on disk, serialized correctly instead of json
-   "Only on memory" should be optional
-   A small query language (so fun)
-   Maybe some kind of lists?
-   Support ints, floats and bools, not just strings (?)
