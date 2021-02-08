# Bite

Minimalistic JSON Key-Value Store.

## How

To set the value,

    S somekeyname Some string as a value I guess
    > OK

To get the value,

    G somekeyname
    > Some string as a value I guess

    G keywithoutvalue
    >

For JSON use the dot notation on keys,

    S library.name Bite
    S library.why Simplest database ever
    S library.author.name Andrés
    S library.author.twitter matnesis

This way you can construct JSON using the keys, with the full path,

    JS library
    >
    {
        "library" : {
            "name" : "Bite",
            "why" : "Simplest database ever"
            "author" : {
                "name" : "Andrés",
                "twitter" : "matnesis
            }
        }
    }

Or just the value,

    J library
    >
    {
        "name" : "Bite",
        "why" : "Simplest database ever"
    }

    J library.why
    > "Simplest database ever."

A **./data/DB.json** file will be created with the information sorted.

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
