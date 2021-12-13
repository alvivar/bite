# Bite

Multi-Thread Key-Value Server database with real time subscriptions.

## Tutorial

To set a value, use **s**.

    s somekeyname Some string as a value
    > OK

To set a value, but only if the key doesn't exist, use **s?**.

    s? somekeyname Update if the key doesn't exists
    > OK

To increase a value by 1, use **+1**. The value become 0 if it isn't a number or
doesn't exist, it returns the result.

    s numberkey 9
    > OK

    +1 numberkey
    > 10

To append a value, use **+**.

    + somelist one
    > one

    + somelist , two
    > one, two

To delete a key and his value, use **d**.

    d somelist
    > OK

To get a value, use **g**.

    s somekey Some string as a value
    > OK

    g somekey
    > Some string as a value

    g keywithoutvalue
    >

A cool thing about **bite**, is that can make a query to get multiple values
from different keys, as long as you use the **dot** notation to connect the
keys as parent/children.

    s data.name Bite
    s data.why Simplest database ever
    s data.author.name Andrés Villalobos
    s data.author.twitter matnesis

This way you can use **b** to get a list of keys and values from the children of
a particular key, separated by the byte 0.

    b data.author
    > name Andrés Villalobostwitter matnesis

A more classic behavior, is that you can construct JSON with **js**.

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

    #g parent.child
    > Value changed because some client set parent.child to something

**#j** sends you the key and the value as JSON.

    #j parent.child
    > { "data" : "Value changed because some client set parent.child.data to something" }

**#b** sends you just the key and the value separated by space.

    #b parent.child
    > id Value changed because some client set parent.child.data.id to something

^ If you subscribe to **parent.child** you will also receive updates from the
children in dot key notation, like **parent.child.data.id**.

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

## Things that I would like to add

- Auth.
- The BTree on disk, serialized correctly instead of json.
- "Only on memory" should be an option.
- You should be able to send several instructions at the same time, and receive responses accordinly?
- Maybe some kind of lists?
- Support ints, floats and bools, everything is a string at the moment.
