# Bite

Minimalistic JSON Key-Value Store.

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

The server runs on **0.0.0.0:1984**, just like me:

    cargo run --release --p server

The client is just a simple test that connects to **127.0.0.1:1984** to send and
receive messages.

    cargo run --release --p client

Or for a custom test server, use the first argument:

    cargo run --release -p client -- 123.45.678.90:1234

You should be able to write from any TCP connection, just send complete lines
before flush.

## Docker installation

It includes the **docker-compose** and **Dockerfile** to build and run the
server.

    docker-compose build
    docker-compose up -d

Then you can connect at **127.0.0.1:1984**.

## Tech

Rust multi-thread **TcpListener** storing on a **BTreeMap** serialized into a
json file with **Serde**. Ready to run on **Docker**.

## Things to do

- Auth.
- Support ints, floats and bools.
- Maybe Lists.
