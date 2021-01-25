# Bite

Minimalistic Key Value Store. A socket waiting for messages to GET and SET
values.

## How to use

To set the value:

    SET somekeyname Some string as a value I guess
    > OK

To get the value:

    GET somekeyname
    > Some string as a value I guess

    GET keywithoutvalue
    >

## Tech

A Rust TcpListener that stores data on a BTreeMap serialized into a file with
Serde.
