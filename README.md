# Bite

Minimalistic JSON Key-Value Database.

## How

To set the value, use **s**.

    s somekeyname Some string as a value I guess
    > OK

To get the value. use **g**.

    g somekeyname
    > Some string as a value I guess

    g keywithoutvalue
    >

For JSON use the dot notation on keys.

    s data.name Bite
    s data.why Simplest database ever
    s data.author.name Andrés Villalobos
    s data.author.twitter matnesis

So you can construct JSON with **js**.

    js data
    >
    {
        "data": {
            "author": {
                "name": "Andrés Villalobos",
                "twitter": "matnesis"
            },
            "name": "Bite",
            "why": "Simplest database ever"
        }
    }

    js data.why
    >
    {
        "data": {
            "why": "Simplest database ever"
        }
    }

Use **j** to get the value without the full path.

    J data
    >
    {
        "author": {
            "name": "Andrés Villalobos",
            "twitter": "matnesis"
        },
        "name": "Bite",
        "why": "Simplest database ever"
    }

    J data.why
    > "Simplest database ever"

Everything will be stored sorted on **data/DB.json**.

## How to run

The server runs on **0.0.0.0:1984**, you can send/receive messages with any
TCP connection, just send complete lines (0xA) before flush.

    cargo run --release --p server

The client is a simple test that connects to **127.0.0.1:1984**, write and hit
enter to send/receive.

    cargo run --release --p client

You could use the first argument to specify a different address.

    cargo run --release -p client -- 123.45.678.90:1234

## Docker

It includes the **docker-compose** and **Dockerfile** to build and run the
server.

    docker-compose build
    docker-compose up -d

Ready to run on **127.0.0.1:1984**.

## Tech

Rust multi-thread **TcpListener** storing on a **BTreeMap** serialized into a
json file with **Serde**.

Ready to run on **Docker** using [Google Container
Tools](https://github.com/GoogleContainerTools/distroless/blob/master/examples/rust/Dockerfile)
to run the binary.

## Things to do

- Auth.
- Support ints, floats and bools.
- Maybe Lists.
- Maybe the BTree on disk.
- Only memory should be optional.
