# Bite

Minimalistic Key Value Store. It's basically a chat, a TcpListener waiting for
messages to GET and SET values.

## How to use

To set the value:

    SET somekeyname Some string as a value I guess
    > OK

To get the value:

    GET somekeyname
    > Some string as a value I guess

    GET keywithoutvalue
    >

## Why

Sometimes I just need a quick and easy server to save data in experiments or prototypes.

## Tech

A Rust TcpListener over a BTreeMap for data, eventually serialized into a file with Serde.
