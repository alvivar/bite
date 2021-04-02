# Bite

Key-Value Server.

## Tutorial

To set a value, use **s**.

    s somekeyname Some string as a value I guess
    > OK

To set a value, but only if the key doesn't exist, use **s?**.

    s? somekeyname Update if the key doesn't exists
    > OK

To get a value, use **g**.

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

### Subscriptions

You can subscribe to a key to receive values in realtime.

**#g** sends you the value.

    #g parentKey.childKey
    > Value changed because some client set parentKey.childKey to something

**#j** sends you the key and the value as JSON.

    #j parentKey.childKey
    > { "childKey" : "Value changed because some client set parentKey.childKey to something" }

^ If you subscribe to **parentKey.childKey** you will also receive updates from
children in dot key notations, like **somekey.subkey.other**, but no from
parents like **parentKey**.

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

The server runs on **0.0.0.0:1984**, you can send/receive messages with any
TCP connection, just send complete lines (0xA) before flush.

    cargo run --release --p server

The client is a simple test that connects to **127.0.0.1:1984**, write and hit
enter to send/receive.

    cargo run --release --p client

You could use the first argument to specify a different address.

    cargo run --release -p client -- 123.45.678.90:1234

## Tech

**Rust** multi-thread **TcpListeners** storing on a **BTreeMap** serialized into a
json file with **Serde**.

Uses [Google Container
Tools](https://github.com/GoogleContainerTools/distroless/blob/master/examples/rust/Dockerfile)
to run the binary on **Docker**.

## Priorities

- Auth.
- Support ints, floats and bools, everything is a string at the moment.
- The BTree on disk, serialized correctly instead of json.
- "Only on memory" should be an option.
- You should be able to send several instructions at the same time, and receive responses accordinly. (?)
- Maybe Lists. (?)
