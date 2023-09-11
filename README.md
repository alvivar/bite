# BITE

Key-Value database with subscriptions, designed for real-time multiplayer applications.

To set a value, use **s**.

    s somekeyname Some string as a value
    > OK

To get a value, use **g**.

    g somekey
    > Some string as a value

    g keywithoutvalue
    >

You can subscribe to a key to get updates when values change. Use **#g** to
receive the value.

    #g parent.child
    > OK

    s parent.child Some value
    > OK

    > Some value (*On all subscribers)

And more commands available, check the [**commands**](Commands.md) for more.

## C# Library

Check out [**.csharp**](/.csharp/) for a simple **C#** client library.

## Environment variables config

The endpoint where the server will listen:

    SERVER=127.0.0.1:1984

If you want to see logs in your console, set the **RUST_LOG** environment to **info**.

    RUST_LOG=info

## Docker

It includes the **docker-compose** and **Dockerfile** to build and run the
server. Just take a look to change the environment variables and security
settings to your needs.

    docker-compose up -d --build

Ready to run on **0.0.0.0:1984** (Docker all available interfaces).

## BITE Protocol

6 bytes as header, then a maximum of 65529 bytes of data, a total of 65535 bytes
together.

    [  2 Bytes  ][  2 Bytes   ][ 2 Bytes ][ Max 65535 - 6 ]
    [ Client Id ][ Message Id ][   Size  ][   Data Bytes  ]

Check out the [**protocol**](Protocol.md) for more details.

## Tech

**Rust** multi-thread **TcpListeners**, using **polling** from **smol** to
handle sockets events, storing on a **BTreeMap** serialized into a json file
with **Serde**.

Uses Google Container Tools
[distroless](https://github.com/GoogleContainerTools/distroless) to run the
binary on **Docker**.

## Things that I would like to add

-   Auth (Soon, working on a Tokio/Warp proxy with security)
-   The BTree on disk, serialized correctly instead of json
-   "Only on memory" should be optional
-   A small query language (so fun)
-   Maybe some kind of lists?
-   Support ints, floats and bools, not just strings (?)
