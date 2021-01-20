# Bite

Minimalist Key Value Store.

It's basically a chat, a TcpListener waiting for messages to GET and SET values.

## How to use

To set the value:

    SET somekeyname Whatever kind of string I guess
    > Ok.

To get the value:

    GET somekeyname
    > Whatever kind of string I guess

To get the value or default:

    GET keyname Some kind of default, just in case
    > Some kind of default, just in case

## Why

Sometimes I just need a quick and easy server to save data between multiplayer experiments.

## Tech

Powered by Rust, basically a TcpListener chat over a BTreeMap, eventually serialized into a file with Serde.
